# Census Archive — OMP

> **ARCHIVED FIGURES ARE NOT CITABLE.** This file is a historical receipts archive, not current ground truth. Re-run the producing command from the source section before use.
>
> Archive snapshot: **2026-09-07**. The source excerpt below is verbatim; its figures are not remeasured or corrected here.
>
> Original measurement dates present in this excerpt: not stated in the excerpt.
>
## The fifth rule: the crates exist to orchestrate OMP, and today they scrape it

Everything in this repo is built to drive OMP. Measured 2026-08-31 against the **installed** source
at `/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent` (v18.0.11, `dist/cli.js` 19 MB),
the crates consume **none of it**. Not a thin subset, not a legacy subset — zero. Every `dist/…` path
below is relative to that install root.

**Two columns, same day. Every number has the command that produces it:**

| surface that exists | command that counts it | measured | we consume |
|---|---|---|---|
| CLI subcommands | `omp --help`, COMMANDS block | **39** | **0** |
| type-surface directories under `dist/types` | `find dist/types -mindepth 1 -maxdepth 1 -type d \| wc -l` | **57** | **0** |
| top-level declaration files beside them | `find dist/types -mindepth 1 -maxdepth 1 -name '*.d.ts' \| wc -l` | **14** | **0** |
| an RPC transport ships — `--mode=<text\|json\|rpc\|rpc-ui>` is a documented top-level flag | `omp --help \| grep -- --mode` | **1 flag, 4 modes** | **0** |
| `omp/*` methods in the bundle | `grep -oE '"omp/[A-Za-z]+"' dist/cli.js \| sort -u` | **3** — `omp/muxConnect`, `omp/muxPing`, `omp/muxRestartServer` | **0** |

57 + 14 = **71 entries** under `dist/types`. Say it that way. An earlier pass published "71
directories"; a worker independently measured 57 and the two disagreed. The reconciliation was that
entries had been counted and called directories. **Neither number was fabricated — the noun attached
to the count was wrong**, which is the same class as every other confident-wrong figure here. The
directories that *are* our lifecycle are named in that tree: `jsonrpc`, `tools`, `slash-commands`,
`commands`, `session`, `task`, `goals`, `plan-mode`, `modes`, `subprocess`, `exec`, `dap`, `debug`,
`capability`, `registry`, `extensibility`, `memories`, `mnemopi`, `memory-backend`, `irc`, `collab`,
`live`, `eval`, `hindsight`, `autolearn`, `autoresearch`, `security`, `secrets`.

**The zero is four greps over `crates/*/src/*`, each printed with the count it returned:**

~~~bash
cd /Users/josh/Developer/omp-orchestrator
for p in 'Command::new("omp")' 'mode=rpc' 'muxConnect' 'omp/'; do
  printf '%s -> %s files\n' "$p" "$(git grep --no-index -lF "$p" -- 'crates/*/src/*' | wc -l | tr -d ' ')"
done
~~~

Measured output — `Command::new("omp")` → **0 files**; `mode=rpc` → **0**; `muxConnect` → **0**;
`omp/` → **0**.

**Positive control, per the second rule.** The identical command shape with a pattern we know is
present returns nonzero: `Command::new("br")` → **3 files**. A zero from a pattern that can never
match is not evidence of absence. `--no-index` is load-bearing, not cosmetic:
`crates/no-shell-gate/src/bin/pre-commit-gate.rs` is untracked, so tracked-only `git grep` reports
**3** `git` spawn sites where the working tree has **4**.

What the crates *do* spawn, same census:

~~~bash
git grep --no-index -hoE 'Command::new\("[a-z_-]+"\)' -- 'crates/*/src/*' | sort | uniq -c | sort -rn
~~~

```
   5 Command::new("br")
   4 Command::new("git")
   1 Command::new("tmux")
   1 Command::new("cargo")
```

`br`, `git`, `tmux`, `cargo`. **No `omp`.** We orchestrate OMP by reading the terminal it drew.

**Every classifier defect measured today is downstream of this one fact** — not correlated with it,
caused by it. Each row names what we do instead of a protocol, and why the protocol makes the defect
unconstructible:

| defect, measured 2026-08-31 | what we do instead | why a protocol removes it |
|---|---|---|
| pane state | a **braille-spinner regex** over `capture-pane` | a state method exists; a spinner is a *rendering*, and we are parsing paint |
| "receiver receipt" | **timer reset + spinner-stripped content hash** ≥75s apart | a typed send returns a delivery response; a hash of glyphs is a guess about one |
| two codex panes read `<no marker>` | last-status-line scan, defeated by a **tool-call box border drawn AFTER the status line** | an artifact of *draw order*. Draw order does not exist over a typed protocol |
| `ntm --robot-send` refuses codex panes with *"cod composer not visible"* (cp-nq2s9) | a **terminal-inspection guard** | a protocol refusal names a *state*; this one names a **visibility**, which is a fact about pixels |
| cp-z42vu: a send returned `success:[4]` while the packet never arrived — and the **inverse** fired today in the pending-dispatch marker | fire-and-hope | both directions are the signature of an **unacknowledged transport**. Ack removes both, not one |

**Of the surface we do not consume, the split that matters** (measured independently and agreeing):

- **(b) reimplemented by scraping — 4:** pane state, dispatch, session, health check. Each has an OMP
  RPC or CLI alternative *that exists today*. These are not gaps; they are rewrites of shipped
  surface, done through a terminal.
- **(c) should use — 5:** `omp/muxConnect`, `omp/muxPing`, `omp/muxRestartServer`, `goals`, `collab`.
  Nothing in `crates/` mentions any of the five.

`omp-orchestrator-omp-surface-map-41b` owns turning this into the per-crate table.

**NO-CLAIM.** "No crate calls OMP" is measured **for our crates only** — the four greps above scan
`crates/*/src/*` in this repo and nothing else. **NTM may itself speak an OMP protocol beneath
`--robot-send`; that is UNMEASURED.** The evidence leans against it — a protocol-level refusal would
not be phrased as *composer visibility*, and a protocol-level receipt would not be reconstructed from
a timer reset — but leaning is not measuring. Until someone reads NTM's send path, the honest claim
is about the boundary we scanned.

**NO-CLAIM, second.** **Mapping a surface is not adopting it.** Some of the scraping is likely
*correct*: a third-party pane (codex, a bare shell) has no OMP RPC to answer, so terminal inspection
is the only channel that exists for it. This rule does not say "replace the scraper." It says the
choice must be **visible** — for each scraped surface, either the typed alternative is named and not
used for a stated reason, or it is used. Silence about a 71-entry surface we touch zero times is the
failure, not the scraping.

---

## OMP lifecycles — what they are and where to find them

OMP (Oh My Pi) v18.0.11 — node CLI "@oh-my-pi/pi-coding-agent", repo "can1357/oh-my-pi". 29 built-in
tools plus 3 hidden (yield, goal, think), 136 slash commands, and **39 CLI subcommands** — counted,
not estimated, from the COMMANDS block of `omp --help`:

~~~bash
omp --help | awk '/^COMMANDS/{f=1;next} f&&/^[[:space:]]*$/{exit} f&&/^  [a-z]/{c++} END{print c}'
~~~

Measured output: `39`.

The installed RPC handler exposes **42 inbound JSON-RPC command methods**; the derivation command and
its output are below. Static production reachability in the **control-plane** adapter — a *different*
repo — is **5/42**. In **this** repo it is **0/42**, and that zero is the fifth rule.

**Retired figure: "81 JSON-RPC methods, and we currently use 17 of the 81."** That pair was
**inherited**, ships **no command that produces it**, and **could not be re-derived** on 2026-08-31
against the installed binary. The reproducible figures are the **42** handler methods below, the
**39** subcommands above, **3** `omp/*`-prefixed methods in the bundle (`omp/muxConnect`,
`omp/muxPing`, `omp/muxRestartServer`), and **57 directories + 14 declaration files** under
`dist/types`. **81 and 17 are retired — cite neither.** And do not read their retirement as a
*smaller* surface: what is measurable is larger than 81 and we consume none of it.
`omp-orchestrator-omp-surface-map-41b` owns producing the real per-crate table.

### Installed RPC command census (measured 2026-08-31)

Version gate and source identity:

  omp --version -> omp/18.0.11
  /Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js
  SHA-256: a95635ad43ab85fcabcbee9bbcc593d9ea8e68ba54228b4c9fdbd1e25766281c; bytes: 19803745.

This command derives the method list from the installed binary's RPC dispatch handler; it is not a
hand-transcribed table:

~~~bash
omp --version && bun -e 'const p="/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js"; const s=await Bun.file(p).text(); const start=s.indexOf("let w=async(v)=>"); const end=s.indexOf("},E=new KWt",start); const methods=[...s.slice(start,end).matchAll(/case"([^"]+)"/g)].map(x=>x[1]); console.log("RPC_COMMAND_METHODS="+methods.length); console.log(methods.join("\n"));'
~~~

Measured output: RPC_COMMAND_METHODS=42.

negotiate_protocol, prompt, steer, follow_up, abort, abort_and_prompt, new_session, switch_session,
branch, get_state, set_fast_mode, get_available_commands, set_todos, set_host_tools,
set_host_uri_schemes, set_subagent_subscription, get_subagents, get_subagent_messages, set_model,
cycle_model, get_available_models, set_thinking_level, cycle_thinking_level, set_steering_mode,
set_follow_up_mode, set_interrupt_mode, compact, set_auto_compaction, set_auto_retry, abort_retry,
bash, abort_bash, get_session_stats, export_html, get_branch_messages, get_last_assistant_text,
set_session_name, handoff, get_messages, get_messages_page, get_login_providers, login.

### Static production reachability (measured 2026-08-31)

Scope: production Rust under /Users/josh/Developer/control-plane/crates/xtask/src/; tests, comments,
and compatibility tables are excluded. This command derives Rust constructor call sites and maps each
constructor through RpcRequest::to_frame to the installed handler method:

~~~bash
bun -e '
const installedPath="/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js";
const installed=await Bun.file(installedPath).text();
const handlerStart=installed.indexOf("let w=async(v)=>");
const handlerEnd=installed.indexOf("},E=new KWt",handlerStart);
const installedMethods=[...installed.slice(handlerStart,handlerEnd).matchAll(/case"([^"]+)"/g)].map(m=>m[1]);
const sourcePath="/Users/josh/Developer/control-plane/crates/xtask/src/omp_rpc.rs";
const source=await Bun.file(sourcePath).text();
const frameStart=source.indexOf("pub fn to_frame");
const frameEnd=source.indexOf("pub fn handshake_requests",frameStart);
const frameSource=source.slice(frameStart,frameEnd);
const variantToMethod=new Map();
for(const match of frameSource.matchAll(/Self::([A-Za-z]+)(?:(?!Self::)[\s\S]){0,800}?"type"\s*:\s*"([^"]+)"/g)) variantToMethod.set(match[1],match[2]);
const start=source.indexOf("pub fn handshake_requests");
const end=source.indexOf("\n}",start);
const rows=[];
for(let lineStart=start;lineStart<end;){const lineEnd=source.indexOf("\n",lineStart);const stop=lineEnd<0||lineEnd>end?end:lineEnd;const match=source.slice(lineStart,stop).match(/RpcRequest::([A-Za-z]+)/);if(match){const method=variantToMethod.get(match[1]);if(!method||!installedMethods.includes(method))throw Error("unmapped RPC constructor: "+match[1]);rows.push(sourcePath+":"+source.slice(0,lineStart).split("\n").length+" "+method)}lineStart=stop+1}
const unique=[...new Set(rows.map(row=>row.slice(row.lastIndexOf(" ")+1)))];
console.log("installed_rpc_commands="+installedMethods.length);
console.log("static_production_rpc_commands="+unique.length+"/"+installedMethods.length);
console.log(rows.join("\n"));
'
~~~

Measured output:

installed_rpc_commands=42
static_production_rpc_commands=5/42
/Users/josh/Developer/control-plane/crates/xtask/src/omp_rpc.rs:275 negotiate_protocol
/Users/josh/Developer/control-plane/crates/xtask/src/omp_rpc.rs:276 get_state
/Users/josh/Developer/control-plane/crates/xtask/src/omp_rpc.rs:277 get_available_commands
/Users/josh/Developer/control-plane/crates/xtask/src/omp_rpc.rs:278 get_available_models
/Users/josh/Developer/control-plane/crates/xtask/src/omp_rpc.rs:279 set_fast_mode

RpcRequest::CancelUiRequest at omp_rpc.rs:740 emits the separate extension_ui_response frame and is
intentionally excluded from the inbound RpcCommand denominator.

This is **static reachability**, not runtime usage. It proves production constructors exist in the scanned
adapter source; it does not prove a live OMP process, provider response, or invocation through an
unscanned path.

### The RPC lifecycle (typed, in crates/xtask/src/omp_rpc.rs in control-plane)

Read the enum, not this table, when precision matters — this is a map to the source.

| State | Meaning |
|---|---|
| `Spawned` | Child started; no `ready` yet |
| `Ready` | `ready` observed **and** it advertised the required version |
| `Negotiated` | `negotiate_protocol` v2 answered successfully |
| `Active` | Handshake complete: every issued request answered, metadata observed |
| `Stopping` | Input closed; awaiting exit |
| `Stopped` | Clean terminal |
| `Failed` | **Restrictive** terminal — see `FailureKind` |
| `TimedOut` | **Restrictive** terminal — a bounded wait elapsed |

Two properties carry the weight:

- **Terminal states admit no further transition.** The machine, not the caller, enforces it.
- **A restrictive terminal is one a caller must not read as success.** `Failed` and `TimedOut` are
  restrictive. This is why *a timeout is not a verdict*: an empty buffer from a killed child must
  map to `TimedOut`, never to the token a genuinely failing subject produces.
- **No wait in the adapter is unbounded, including shutdown.**

Supporting types: `LifecycleMachine` (transitions), `LifecycleReport` (the observable outcome of one
run), `TimeoutPhase` (which bounded wait elapsed), `FailureKind` (why a restrictive terminal).

### The pane lifecycle (what an operator sees)

Distinct from the RPC lifecycle and more often wrong, because it is read from a terminal.

**The v18 status-line contract, measured 2026-08-31:**

- **Working** — a braille spinner followed by an **elapsed timer** (`⠸ 4m`)
- **Idle** — the `π` prompt glyph where the spinner would be

The shipped NTM presets required the literal word `Working`, which v18 **never renders**. The
classifier scored **0/3 on live payload** at 03:08Z and **3/3** after the fix (`d05200c`).

**Read the LAST status line, never the buffer.** A whole-buffer scan matches a stale spinner still
in scrollback: one pane scored *working AND idle simultaneously* while genuinely idle.

**Two captures or it is not a claim.** `Working (27s)` and a frozen pane render identically. Compare
timer **and** spinner-stripped content hash ≥75s apart.

**`safe_to_dispatch` is not liveness.** A wedged pane accepts a packet, parks it at
`Press up to edit queued messages`, and never submits it.

### The bead lifecycle (the unit of work)

`open → in_progress (claimed) → closed (with cited evidence)`, with two traps that are *ours*, both
measured:

1. **The close reason must start with** `MUTATION-VERIFIED` / `DONE` / `APPROVED` / `WONTFIX`.
   A prose reason is refused by policy, the refusal scrolls past, and the agent believes it landed.
2. **A child blocked by its parent epic cannot close.** An epic closes *after* its children, so that
   dependency is inverted and makes both permanently unclosable. `--force` with the reason recorded
   is correct when the epic is the only blocker.

---


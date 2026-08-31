# AGENTS.md — omp-orchestrator

The operating manual for any agent working this repo. `README.md` says what the product is and why.
This file says how you work here, what every crate is for, and what "done" means.

---

## The one rule

**No `.sh`. No `.py`.** Not in `bin/`, not in `scripts/`, not "just for testing." A Rust gate walks
`git ls-files` and fails the build on either extension. If you reach for a shell script, you have
found a missing crate.

The exemption list is empty. There is deliberately no `check.sh` carve-out — that carve-out is what
let **160 scripts and 60,467 lines** accrete in the repo this substrate is extracted from.

---

## The second rule: BUILT ≠ WIRED

A mechanism that is written, tested, adversarially hardened, and **invoked by nothing** is worth
zero. Green tests on an unwired lane are not evidence; they are a receipt for work nobody consumes.

We take the mechanism from `franken_lean` (`crates/fln-conformance/tests/contract_roots.rs`, found
via `fh`), and it is the shape to copy:

```rust
/// Lanes that exist and are correct but are deliberately not yet wired.
const UNWIRED_LANE_ALLOWANCE: &[(&str, &str)] = &[];
```

**An empty allowlist.** Every lane must be wired; an exception is a *named row with a reason*, not
silence. A conformance test walks the declared lanes and fails on any that no caller invokes.

**In this repo that means:** every crate that declares a gate, check, or lane ships a test proving
a real caller reaches it — a CI job, a subcommand, another crate. `fh N043` is us failing this
exact way: *"BUILT ≠ WIRED aimed at ourselves, and we ran the full battery of verification rituals
without it firing once."*

**Wiring proof needs a positive control.** Grep for something you *know* is wired and confirm it
hits. A zero from a pattern that can never match is not evidence of absence.

---

## OMP lifecycles — what they are and where to find them

OMP (Oh My Pi) v18.0.11 — node CLI "@oh-my-pi/pi-coding-agent", repo "can1357/oh-my-pi". 29 built-in
tools plus 3 hidden (yield, goal, think), 136 slash commands, ~40 CLI subcommands.
The installed RPC handler exposes **42 inbound JSON-RPC command methods**. Static production source
reachability in the control-plane adapter is **5/42**; the old "17 of 81" claim is not reproduced by
the installed binary and is replaced below with a derivation command and its output.

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

## The four skills, and how they compose here

Not four checklists — one philosophy with four entry points. Each has one sentence that binds.

### `/planning-workflow` — converge before you build
**Plan-space is ~25× cheaper than code-space.** Debates belong in planning, before the swarm burns
implementation tokens. Three reasoning spaces: plan (architecture, cheapest to change), bead (task
boundaries, ~5× plan to rework), code (~25× plan). Don't answer plan-space questions in code-space.

### `/beads-workflow` — the bead is the spec
**"Check your beads N times, implement once."** Every bead is self-contained with **testable
acceptance: run X, expect Y**. A bead you cannot write acceptance for is not granular enough.

*Why this is load-bearing here:* a bead without acceptance cannot be **worked**, only **adjudicated**
— and adjudication reliably produces "no work to be done" instead of work. Measured: a P0 bead at
the head of the ready queue had **no ACCEPTANCE section at all**; two agents in a row triaged it and
went idle rather than shipping.

### `/beads-bv` — the DAG decides the lane
PageRank over the dependency graph. **Work the articulation points, not the comfortable leaves.**
Easy-bead cherry-picking while critical-path work starves is a named pathology, not a preference.

### `/vibing-with-ntm` — observe before you nudge, and police the credit
Two rules, both binding:

> **One Rule.** A pane is not stuck, idle, limited, blocked, or finished until pane truth, robot
> state, work state, and artifact evidence **agree**.

> **Second Rule.** The swarm is paid in credit and will counterfeit credit if you let it. Process
> artifacts are not progress, refusals are not delivery, commits are not a KPI, and a close without
> cited evidence is a debt.

**DO NOT POLL. BLOCK.** Repeated activity checks on a timer are the anti-pattern; NTM ships blocking
waits that fire on a state transition. Tails verify a post-condition on one pane — they never
*discover* that something changed.

### `/brennerbot-with-ntm` — delete hypothesis space, don't accumulate evidence

> A session is **a machine for deleting hypothesis space cheaply**, not a machine for accumulating
> evidence. Maximize (expected mind-change × downstream option value) / (time × cost × ambiguity).
> When two phases compete, the one that kills more candidate hypotheses per token wins.

**No falsifier means no session.** Prefer **refuters over supporters** — evidence that could kill
your hypothesis is worth more than evidence consistent with it. Generate ≥3 hypotheses including a
**forced third alternative**, then attack the survivors.

*Applied here:* when a lane misbehaves, write the falsifier first. Tonight's dispatcher bug survived
three rounds of hypothesising and died in one `bash -x` — because the trace could refute, and the
theories could only agree with themselves.

### How they compose

```
/planning-workflow   converge the plan      ─┐
/beads-workflow      plan -> testable beads  ├─ before any agent is dispatched
/beads-bv            DAG says which bead    ─┘
/vibing-with-ntm     dispatch + police credit ─── during the wave
/brennerbot          when something is wrong  ─── falsifier first, refuters over supporters
```

---

## The crates: what each one is and why it exists

The source-backed table below records 24 current control-plane crate rows and 32,087 Rust LOC. 22 of
the 24 have a `tests/` directory. The current `omp-orchestrator` workspace is a separate, partial
extraction and loads 8 packages. Counts are derived from the source roots, not copied from the
descriptions. Grouped by the lifecycle stage they serve.

### Ground truth — "what is actually true right now"

These exist because **every classifier we trusted has been wrong at least once**, and a wrong
liveness read either interrupts real work or leaves a worker idle beside a full queue.

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `pane-truth` | 1247 | Ground-truth tmux pane state | The shell version remains the differential oracle; this is the typed reading |
| `fleet-truth` | 1621 | Fleet-wide inspection register | One place answers "what is the fleet doing" so callers stop re-deriving it |
| `fleet-reconcile` | 1424 | NTM projection vs tmux reality | NTM's snapshot returns `total_sessions: 0` with `success: true` when stale; tmux does not lie |
| `oracle-compare` | 449 | Shared comparator: claim vs independent oracle | An empty or unreadable oracle must be an ERROR, never a silent agreement |
| `pane-oracle-diff` | 741 | tmux pane census vs ntm projection | Catches projection drift before a dispatch rides it |
| `oracle-pane-state-differential` | 613 | session:index pane-set differential (tmux vs ntm) | Uses the shared set comparator; this source has no Z3 implementation |
| `fleet-composite` | 1372 | Geometric fleet-health composite and diagnostic CLI | Refuses malformed, empty, and non-finite inputs instead of inventing a score |

### Readiness and admission — "may this pane receive work"

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `pane-dispatch-ready` | 1555 | Can this pane SAFELY receive a dispatch | `safe_to_dispatch` is not liveness |
| `pane-dispatch-fence` | 468 | Cross-process per-pane admission fence | Two dispatchers landing during a `/clear` vaporise the packet |
| `composer-typed` | 556 | Does the composer hold real TYPED text | Sender success is not receiver receipt |
| `ntm-fleet-monitor` | 3122 | Typed fleet actions + approval waves. **Classifies; does not send** | Separating classification from actuation makes the verdict auditable |

### Selection — "what should be worked next"

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `loop-queue-filter` | 912 | Fail-closed queue selector | Epics invite unbounded scope; in-flight work must not be re-offered |
| `loop-coverage` | 926 | Typed coverage matrix. **A map, not a gate** | Says honestly what is *not* covered rather than implying completeness |
| `refill-idle-panes` | 842 | Refill every idle pane from the bv DAG | An idle worker beside a ready queue is the conductor's failure |
| `omp-idle-dispatch` | 1667 | Fail-closed idle OMP pane dispatch lane | Makes repository, session, ledger, and admission inputs explicit before dispatch |

### Dispatch — "send the work"

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `fast-dispatch` | 2292 | Admit on a fresh standing verdict, select free panes | Must fail closed on a stale verdict |
| `tick-dispatch` | 990 | Ground-truth pane dispatch fence | Decided by tmux/ntm truth, not a cached label |
| `loop-driver` | 2484 | Single-instance, deadline-bounded driver | Two ticks fighting over one pane is corruption |
| `loop-tick` | 1480 | Single-pane dispatch tick | The unit the driver repeats |
| `fleet-monitor` | 2569 | OBSERVE lane: attention wait + idle/ready scan | Block on a state transition; polling is the anti-pattern |

### Verification and reaping — "did it actually happen"

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `verify-dispatch` | 1291 | Verification from **bead status only** | Ground truth, never a pane's self-report |
| `dispatcher-deadman` | 883 | Watchdog: eligible work that received no packet | The failure that is invisible because everything looks healthy |
| `reap-finished-panes` | 1189 | Sweep finished panes before the next dispatch | An unreaped pane is capacity that silently disappears |
| `wired-but-inert-guard` | 1394 | Fail-closed proof that declared dispatch gates are actually invoked | Prevents a green unused gate from counting as coverage |

**Dependency shape** (from each `Cargo.toml`, current 24-row table): 17 leaves with zero path deps;
7 with exactly one — `ntm-fleet-monitor` → `loop-coverage`, `fleet-monitor` →
`ntm-fleet-monitor`, `pane-oracle-diff` → `oracle-compare`,
`oracle-pane-state-differential` → `oracle-compare`, `tick-dispatch` → `oracle-compare`,
`fast-dispatch` → `loop-switch`, and `loop-driver` → `loop-switch`. **Extract leaves first.**

**Unsafe posture in the current 24-row table: 5 of 24.** `ntm-fleet-monitor`,
`refill-idle-panes`, `omp-idle-dispatch`, `wired-but-inert-guard`, and `fleet-composite`
declare `unsafe_code = "forbid"`. The 815 extraction scope is 23 crates and is also 5-for-23;
the historical 815 comment claiming 3-for-23 is stale after control-plane commit `8fc3e4b`, which
added the lint to the other two ported crates. A crate that will not compile under the lint is a
**finding**, not a reason to drop the lint.

**Measured set reconciliation (2026-08-31).** The pre-audit table had 21 rows, not 20. It included
the real `oracle-pane-state-differential` crate. The three ported crates named by bead
`omp-orchestrator-815` bring the documented table to 24 rows, while 815's stated 23-crate
extraction scope is its original 20 rows plus those three and therefore excludes
`oracle-pane-state-differential`. That is a real scope mismatch, not a rounding issue.

- Target workspace `/Users/josh/Developer/omp-orchestrator`: 8 loaded Cargo packages.
- Source workspace `/Users/josh/Developer/control-plane`: 58 tracked top-level crate manifests;
  Cargo loads 57 packages. The excluded top-level manifest is `crates/loop-tick/Cargo.toml`,
  which declares its own `[workspace]`; the two other tracked manifests are fixture manifests.
- Working-tree source totals for the current 24-row table: 32,087 Rust LOC and 22 crate-level
  `tests/` directories. The 815 23-crate scope totals 31,474 Rust LOC and 21 `tests/`
  directories under the same counting rule.

The audit is re-runnable from the target repo with the source root explicit:

```bash
# Target package count; run in /Users/josh/Developer/omp-orchestrator.
/Users/josh/.cargo/bin/cargo metadata --no-deps --format-version 1 \
  | jq '[.packages[].manifest_path | select(test("/crates/[^/]+/Cargo.toml$"))] | length'

# Source package count; run in /Users/josh/Developer/control-plane. The warnings are meaningful.
/Users/josh/.cargo/bin/cargo metadata --no-deps --format-version 1 \
  | jq '[.packages[].manifest_path | select(test("/crates/[^/]+/Cargo.toml$"))] | length'

# Every documented row -> source files and working-tree Rust LOC.
bun -e 'const s=await Bun.file("AGENTS.md").text(); const start=s.indexOf("## The crates:"); const a=s.slice(start,s.indexOf(String.fromCharCode(10)+"## Use fh",start)); const ns=a.split(String.fromCharCode(10)).filter(x=>x.startsWith("| "+String.fromCharCode(96))).map(x=>x.split("|")[1].trim().slice(1,-1)); for(const n of ns){const d="/Users/josh/Developer/control-plane/crates/"+n; const p=Bun.spawnSync(["find",d,"-type","f","-name","*.rs","-print"]); const fs=new TextDecoder().decode(p.stdout).trim().split(String.fromCharCode(10)).filter(Boolean); let loc=0; for(const f of fs){const t=await Bun.file(f).text(); loc+=t.split(String.fromCharCode(10)).length-(t.endsWith(String.fromCharCode(10))?1:0)} console.log(n+String.fromCharCode(9)+loc+String.fromCharCode(9)+fs.join(","))}'
```

**Source audit result (control-plane `src/lib.rs`/`src/main.rs`, unless noted):**

- **CONFIRMED** — `pane-truth`: pane rules, external command output, and two-capture timing are present.
- **CONFIRMED** — `fleet-truth`: fleet sensors and truth-row rendering are present.
- **CONFIRMED** — `fleet-reconcile`: tmux/NTM reconciliation, typed verdicts, and self-test are present.
- **CONFIRMED** — `oracle-compare`: count/set verdicts and unreadable/empty-arm handling are present.
- **CONFIRMED** — `pane-oracle-diff`: agent-pane census and NTM projection comparison are present.
- **DIVERGENT** — `oracle-pane-state-differential`: it compares session:index `BTreeSet` values through `oracle-compare`; no Z3 dependency or Z3 implementation is present. The table row now states the implementation rather than the stale label.
- **CONFIRMED** — `pane-dispatch-ready`: busy, agent, quota, composer, and motion checks feed admission classification.
- **CONFIRMED** — `pane-dispatch-fence`: per-session/per-pane lock acquisition and release are implemented.
- **CONFIRMED** — `composer-typed`: marker/ANSI-aware typed-composer parsing and self-test are implemented.
- **CONFIRMED** — `ntm-fleet-monitor`: typed actions and approval/refusal wave rendering are implemented; the binary does not send.
- **CONFIRMED** — `loop-queue-filter`: runtime-configured, fail-closed queue filtering is implemented.
- **CONFIRMED** — `loop-coverage`: proof levels, loop layers, edge cases, and reuse authorities form a coverage map, not a gate.
- **CONFIRMED** — `refill-idle-panes`: pane survey, refusal classification, recommendation parsing, and bounded assignment planning are implemented.
- **CONFIRMED** — `fast-dispatch`: fresh-verdict admission, free-pane selection, bounded children, and lock/ledger handling are implemented.
- **CONFIRMED** — `tick-dispatch`: ground-truth pane, discovery, readiness, and send decisions are implemented.
- **CONFIRMED** — `loop-driver`: single-instance locking and deadline-bounded driver output are implemented.
- **CONFIRMED** — `loop-tick`: single-pane dispatch decisions, bounded child execution, and lock acquisition are implemented; its standalone `[workspace]` manifest is the inventory caveat above.
- **CONFIRMED** — `fleet-monitor`: observe wait, idle/ready scan, and standing-verdict writing are implemented.
- **CONFIRMED** — `verify-dispatch`: bead-status-only verification and differential CLI behavior are implemented.
- **CONFIRMED** — `dispatcher-deadman`: eligible-work/no-packet watchdog behavior is implemented.
- **CONFIRMED** — `reap-finished-panes`: finished-pane sweep and bounded external probes are implemented.
- **CONFIRMED** — `omp-idle-dispatch`: fail-closed idle-pane dispatch with typed repository/config inputs is implemented.
- **CONFIRMED** — `wired-but-inert-guard`: tracked caller discovery, gate scans, fail-closed empty-scan handling, and diagnostic commands are implemented.
- **CONFIRMED** — `fleet-composite`: four-factor geometric scoring, malformed-input refusal, and diagnostic CLI behavior are implemented.

The four names shared by both repositories were checked explicitly: `composer-typed`,
`fleet-composite`, and `loop-queue-filter` are byte-identical between
`/Users/josh/Developer/control-plane/crates/<name>` and
`/Users/josh/Developer/omp-orchestrator/crates/<name>`; `pane-dispatch-fence` has the same
purpose but differs in both `Cargo.toml` and `src/main.rs` (the target adds
`subprocess-contract`).

This source audit finds one description divergence and the explicit set/inventory mismatches above;
it does not establish runtime correctness, wiring, or future drift.

---

## Use fh before you build anything

`fh` is the queryable index over our own measured doctrine and Jeffrey's 180-repo mirror. **Ask it
before writing a crate, a gate, or a process.** Re-deriving what we own is the largest token waste
this fleet has.

```bash
fh suggest "<what you are about to build>"   # ranked rows
fh why <row-id>                              # provenance before you believe it
```

Four row types answer different questions:

- **CAPABILITY** — depend on this crate instead of writing it; names what of ours it replaces
- **DOCTRINE** — a measured failure with path + quote + line; the mistake we already paid for
- **BEAD** — current task intent with exact `.beads/issues.jsonl#id` provenance
- **DOC** — pinned repository guidance with source revision and verbatim evidence

**Rows already governing this repo:**

| Row | Governs |
|---|---|
| `C38` | A fixture drifted from production certifies nothing — its green is indistinguishable from a working check |
| `C112` | An ownership claim must name something that **dies with the thing it owns** — a pid in a marker file written by a transient shell dies with the shell |
| `N043` | BUILT ≠ WIRED aimed at ourselves: a full battery of verification rituals that never fired once |
| `N040` | A replacement claim needs a smoke check at **both** ends — the crate installs clean **and** the old caller is gone |

`fh` reports a `STALE` banner when its ledger is older than its threshold. **Read it and say so** —
a stale row is still evidence, but its age is part of the citation.

---

## The asupersync contract (binding)

Every subprocess — `tmux`, `ntm`, `br`, `bv`, a build — is cancellable work with a deadline. Built
on **asupersync 0.4.9** (`/Volumes/ZestData/dicklesworthstone-mirror/asupersync`, a real `[lib]`).

- **`&Cx` first** in every async API we own; `cx.checkpoint()` in loops, retries, long handlers.
- **Region-owned tasks** (`Cx::spawn` / `Scope`). **No detached tasks.**
- **Kill the process GROUP, never the pid.** Measured: orphaned grandchildren (`ppid=1`, 0.0% CPU)
  held the admission lock, so every timeout guaranteed the next attempt failed too — the failure
  created the condition for its own repetition.
- **Drain both pipes.** Undrained stdout+stderr with a `try_wait()` poll deadlocks past ~64 KiB.
  The tell is **0% CPU with no children**; widening the timeout hides it longer.
- **A timeout is not a verdict** — see the restrictive terminals above.

Load `/asupersync-mega-skill` before touching spawn, cancellation, or scheduling code.

---

## Every gate proves it bites

1. **Fires-on-known-bad.** A gate that has never fired on a bad input is not evidence of anything.
2. **A known-GOOD leg is mandatory.** An attack-only suite ships an over-strict gate, and an
   over-strict gate gets routed around — a slower death than no gate.
3. **A mutation leg.** Break the thing the gate keys on; the leg must go RED. Restore
   byte-identically. If it stays green, the leg is not attributable and proves nothing.
4. **Anti-vacuity.** An empty scan set is an **ERROR**, never a pass. A deliverable never checked
   reports identically to one that passed.
5. **State the claim as a floor-raise.** Say what the gate mechanically enforces *and* what still
   passes. A residual "guarantees / proves / makes impossible" in a gate header is itself a defect —
   the overclaim is worse than the gap, because a reader stops looking.

---

## Working here

**Beads.** This repo has its own `.beads` (prefix `omp-orchestrator`). `br` reads the cwd — **cd
first**. Never file substrate work in control-plane's tracker.

**Verification** (`/beads-compliance-and-completion-verification`): status is a **claim**, not a
fact. **Re-run, don't read** — a test that passed in CI yesterday is inadmissible. A test passes
meaningfully only when it (a) exists, (b) exits 0, **and** (c) asserts non-trivially against the
production path. Most theater is (c).

**Never silence stderr** in a command whose output you will cite as evidence.

**Search before building.** `mcp__socraticode__codebase_search` by meaning, then `fh suggest`.
`grep` is the follow-up that jumps to a line, never the opening move.

**Commits.** Path-scoped with an explicit list — `git commit -- <paths>`. Never `-A`; a shared
checkout means a bare commit sweeps in another agent's unfinished work. Commit messages carry a
verification-level tag (`[test]`, `(code-first, test pending)`, `[selftest-verified]`).

---

## Honest limits

- Nothing here is installed. The binary does not exist.
- The crate table is now source-audited against control-plane `src/lib.rs`/`src/main.rs` and
  re-runnable inventory commands. The audit establishes description alignment only; it does not
  establish runtime correctness, wiring, or future source drift.
- The no-shell gate covers **file extensions**. It does not prove no crate shells out at runtime via
  `std::process::Command` — a separate, unbuilt check.
- The unwired-lane conformance test described above is **doctrine here, not yet code**. Until it
  exists, "wired" is checked by hand.

## Grading gate: no bead closes on its own author's word

**A bead is closed by an agent who did NOT implement it.** Verification runs
`/beads-compliance-and-completion-verification` against the bead's own acceptance
criteria, and the close reason cites what the GRADER re-executed — not what the
implementer reported.

Measured 2026-08-31: of the first three closes, one was independently graded (`-4ak`,
pane 1, "re-run, not read") and two were self-certified by their implementer (`-7ai`
GoldLark, `-a3p` BlueLantern). Both self-closes carried real evidence and a NO-CLAIM
line, which is why this is a process gap and not a fabrication — but a report is a
CLAIM, and the whole point of the grade is that a second agent ran the command.

**Stage order, and grading outruns new work.** When a bead is implementation-complete,
grading it takes priority over claiming anything new. A verification backlog is worse
than an empty queue: unclosed finished work makes `br ready` keep serving it, which is
how a pane ends up correctly reporting NO_ELIGIBLE_TARGET and going idle.

Every bead carries a stage, and dispatches name it:

```
IMPL     implementation-complete, commit landed
  ->  GRADING   a DIFFERENT agent re-executes the acceptance criteria
  ->  CLOSED    the grader closes; reason starts MUTATION-VERIFIED / DONE / APPROVED / WONTFIX
```

The close policy REFUSES a prose reason, and the refusal scrolls past in-pane while the
agent moves on believing the close landed. Always read the status back:
`br show <id> --json | jq -r '.[0].status'` — note `br show` returns a BARE list, while
`br list` wraps its rows in `.issues`.

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

OMP (Oh My Pi) v18.0.11 — node CLI `@oh-my-pi/pi-coding-agent`, repo `can1357/oh-my-pi`. 29 built-in
tools plus 3 hidden (`yield`, `goal`, `think`), 136 slash commands, **81 JSON-RPC methods**, ~40 CLI
subcommands. We currently use 17 of the 81.

### The RPC lifecycle (typed, in `crates/xtask/src/omp_rpc.rs` in control-plane)

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

20 crates, 25,567 LOC, extracted from `control-plane/crates/`. 18 arrive with tests. Grouped by the
lifecycle stage they serve.

### Ground truth — "what is actually true right now"

These exist because **every classifier we trusted has been wrong at least once**, and a wrong
liveness read either interrupts real work or leaves a worker idle beside a full queue.

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `pane-truth` | 804 | Ground-truth tmux pane state | The shell version remains the differential oracle; this is the typed reading |
| `fleet-truth` | 1556 | Fleet-wide inspection register | One place answers "what is the fleet doing" so callers stop re-deriving it |
| `fleet-reconcile` | 1396 | NTM projection vs tmux reality | NTM's snapshot returns `total_sessions: 0` with `success: true` when stale; tmux does not lie |
| `oracle-compare` | 449 | Shared comparator: claim vs independent oracle | An empty or unreadable oracle must be an ERROR, never a silent agreement |
| `pane-oracle-diff` | 725 | tmux pane census vs ntm projection | Catches projection drift before a dispatch rides it |
| `oracle-pane-state-differential` | — | Z3 differential on pane_state | Formal check that two state sources agree |

### Readiness and admission — "may this pane receive work"

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `pane-dispatch-ready` | 1454 | Can this pane SAFELY receive a dispatch | `safe_to_dispatch` is not liveness |
| `pane-dispatch-fence` | 417 | Cross-process per-pane admission fence | Two dispatchers landing during a `/clear` vaporise the packet |
| `composer-typed` | 556 | Does the composer hold real TYPED text | Sender success is not receiver receipt |
| `ntm-fleet-monitor` | 3122 | Typed fleet actions + approval waves. **Classifies; does not send** | Separating classification from actuation makes the verdict auditable |

### Selection — "what should be worked next"

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `loop-queue-filter` | 858 | Fail-closed queue selector | Epics invite unbounded scope; in-flight work must not be re-offered |
| `loop-coverage` | 926 | Typed coverage matrix. **A map, not a gate** | Says honestly what is *not* covered rather than implying completeness |
| `refill-idle-panes` | 735 | Refill every idle pane from the bv DAG | An idle worker beside a ready queue is the conductor's failure |

### Dispatch — "send the work"

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `fast-dispatch` | 2073 | Admit on a fresh standing verdict, select free panes | Must fail closed on a stale verdict |
| `tick-dispatch` | 907 | Ground-truth pane dispatch fence | Decided by tmux/ntm truth, not a cached label |
| `loop-driver` | 2429 | Single-instance, deadline-bounded driver | Two ticks fighting over one pane is corruption |
| `loop-tick` | 1472 | Single-pane dispatch tick | The unit the driver repeats |
| `fleet-monitor` | 2503 | OBSERVE lane: attention wait + idle/ready scan | Block on a state transition; polling is the anti-pattern |

### Verification and reaping — "did it actually happen"

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `verify-dispatch` | 1189 | Verification from **bead status only** | Ground truth, never a pane's self-report |
| `dispatcher-deadman` | 875 | Watchdog: eligible work that received no packet | The failure that is invisible because everything looks healthy |
| `reap-finished-panes` | 1121 | Sweep finished panes before the next dispatch | An unreaped pane is capacity that silently disappears |

**Dependency shape** (from each `Cargo.toml`): 14 leaves with zero path deps; 6 with exactly one —
`ntm-fleet-monitor` → `loop-coverage`, plus `fleet-monitor`, `pane-oracle-diff`, `tick-dispatch`,
`fast-dispatch`, `loop-driver`. **Extract leaves first.**

**Unsafe posture on arrival: 2 of 20.** Only `ntm-fleet-monitor` and `refill-idle-panes` declare
`unsafe_code = "forbid"`. A crate that will not compile under the lint is a **finding**, not a
reason to drop the lint.

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
- The crate table is derived from `Cargo.toml` descriptions and LOC counts, **not** from reading all
  25,567 lines. Treat per-crate "why" lines as stated intent until verified — bead
  `omp-orchestrator-6gq` closes this.
- The no-shell gate covers **file extensions**. It does not prove no crate shells out at runtime via
  `std::process::Command` — a separate, unbuilt check.
- The unwired-lane conformance test described above is **doctrine here, not yet code**. Until it
  exists, "wired" is checked by hand.

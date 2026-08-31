# AGENTS.md — omp-orchestrator

The operating manual for any agent working this repo. `README.md` says what the product is and why;
this file says how you work here and what every crate is for.

---

## The one rule

**No `.sh`. No `.py`.** Not in `bin/`, not in `scripts/`, not "just for testing." A Rust gate walks
`git ls-files` and fails the build on either extension. If you find yourself reaching for a shell
script, you have found a missing crate.

The exemption list is empty. There is no `check.sh` carve-out here — that carve-out is what let 160
scripts and 60,467 lines accrete in the repo this substrate is being extracted *from*.

---

## What we are building

A single installable binary that drives a project **plan → beads → bv triage → OMP agent dispatch →
ground-truth verification → close**. Not a fleet monitor, not a dispatcher — the whole lifecycle.

**control-plane is the proving ground.** Nothing here deploys to another repo until it is proven
there. Building is not shipping.

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
| `fleet-truth` | 1556 | Fleet-wide inspection register | One place that answers "what is the fleet doing", so callers stop re-deriving it |
| `fleet-reconcile` | 1396 | NTM projection vs tmux reality | NTM's snapshot returns `total_sessions: 0` with `success: true` when stale; tmux does not lie |
| `oracle-compare` | 449 | Shared comparator: claim vs independent oracle | An empty or unreadable oracle must be an ERROR, never a silent agreement |
| `pane-oracle-diff` | 725 | tmux agent-pane census vs ntm projection | Catches the projection drifting from reality before a dispatch rides it |
| `oracle-pane-state-differential` | — | Z3 differential on pane_state | Formal check that two state sources agree |

### Readiness and admission — "may this pane receive work"

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `pane-dispatch-ready` | 1454 | Can this pane SAFELY receive a dispatch | `safe_to_dispatch` is **not** liveness — a wedged pane accepts a packet and never submits it |
| `pane-dispatch-fence` | 417 | Cross-process per-pane admission fence | Two dispatchers landing in one pane during a `/clear` vaporises the packet |
| `composer-typed` | 556 | Does the composer hold real TYPED text | Sender success is not receiver receipt; verify at the receiving end |
| `ntm-fleet-monitor` | 3122 | Typed fleet actions + approval waves. **Classifies; does not send** | Separating classification from actuation is what makes the verdict auditable |

### Selection — "what should be worked next"

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `loop-queue-filter` | 858 | Fail-closed dispatch queue selector | Epics invite unbounded scope; in-flight work must not be re-offered |
| `loop-coverage` | 926 | Typed coverage matrix for the loop. **A map, not a gate** | Says honestly what is and is not covered rather than implying completeness |
| `refill-idle-panes` | 735 | Refill every idle pane from the bv DAG | An idle worker beside a ready queue is the conductor's failure, not the worker's |

### Dispatch — "send the work"

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `fast-dispatch` | 2073 | Admit on a fresh standing verdict, select free panes | The fast lane; must fail closed on a stale verdict |
| `tick-dispatch` | 907 | Ground-truth pane dispatch fence | Dispatch decided by tmux/ntm truth, not a cached label |
| `loop-driver` | 2429 | Single-instance, deadline-bounded driver | Single-instance or two ticks fight over one pane |
| `loop-tick` | 1472 | Single-pane dispatch tick | The unit the driver repeats |
| `fleet-monitor` | 2503 | The OBSERVE lane: attention wait + idle/ready scan | Block on a state transition; polling is the anti-pattern |

### Verification and reaping — "did it actually happen"

| Crate | LOC | What it does | Why it exists |
|---|---:|---|---|
| `verify-dispatch` | 1189 | Verification from **bead status only** | Ground truth, never a pane's self-report |
| `dispatcher-deadman` | 875 | Watchdog: eligible work that received no packet | The failure that is invisible because everything looks healthy |
| `reap-finished-panes` | 1121 | Sweep finished panes before the next dispatch | A finished pane that is never reaped is capacity that silently disappears |

**Dependency shape (measured from each `Cargo.toml`):** 14 crates are leaves with zero path
dependencies. 6 have exactly one: `ntm-fleet-monitor` → `loop-coverage`, plus `fleet-monitor`,
`pane-oracle-diff`, `tick-dispatch`, `fast-dispatch`, `loop-driver`. Extract leaves first.

**Unsafe posture on arrival: 2 of 20.** Only `ntm-fleet-monitor` and `refill-idle-panes` declare
`unsafe_code = "forbid"`. Bringing the other 18 up is a P0 gate, and a crate that will not compile
under the lint is a **finding**, not a reason to drop it.

---

## The asupersync contract (binding)

Every subprocess — `tmux`, `ntm`, `br`, `bv`, a build — is cancellable work with a deadline. Built
on **asupersync 0.4.9** (`/Volumes/ZestData/dicklesworthstone-mirror/asupersync`, a real `[lib]`).

- `&Cx` first in every async API we own; `cx.checkpoint()` in loops, retries, long handlers.
- Region-owned tasks (`Cx::spawn` / `Scope`). **No detached tasks.**
- **Kill the process GROUP, never the pid.** Orphaned grandchildren (`ppid=1`, 0.0% CPU) held the
  admission lock; every timeout then guaranteed the next attempt failed too.
- **Drain both pipes.** Undrained stdout+stderr with a `try_wait()` poll deadlocks past ~64 KiB.
  The tell is **0% CPU with no children** — widening the timeout hides it longer.
- **A timeout is not a verdict.** An empty buffer maps to `TIMEOUT`, never to the token a genuinely
  failing subject produces.

Load `/asupersync-mega-skill` before touching spawn, cancellation, or scheduling code.

---

## Working here

**Beads.** This repo has its own `.beads` (prefix `omp-orchestrator`). Never file substrate work in
control-plane's tracker. Every bead carries **testable acceptance: run X, expect Y** — a bead
without one cannot be worked, only adjudicated, and adjudication produces "no work to be done."

**Verification.** `/beads-compliance-and-completion-verification`: status is a **claim**, not a fact.
**Re-run, don't read** — a test that passed in CI yesterday is inadmissible. A test passes
meaningfully only when it (a) exists, (b) exits 0, **and** (c) asserts non-trivially against the
production path.

**Every gate proves it bites.** Fires-on-known-bad *and* a known-good leg — an attack-only suite
ships an over-strict gate that gets routed around, which is a slower death than no gate. Add a
mutation that turns the leg RED, and restore byte-identically.

**Anti-vacuity.** An empty scan set is an ERROR. A deliverable never checked reports identically to
one that passed.

**Never silence stderr** in a command whose output you will cite as evidence.

**Search before building.** `mcp__socraticode__codebase_search` by meaning, then `fh suggest`.
We own 56 crates and a 180-repo mirror; re-deriving what exists is the largest token waste we have.
`grep` is the follow-up that jumps to a line, never the opening move.

**Commits.** Path-scoped with an explicit list — `git commit -- <paths>`. Never `-A`. A shared
checkout means a bare commit sweeps in another agent's unfinished work.

---

## Honest limits

- Nothing here is installed anywhere. The binary does not yet exist.
- The crate table is derived from `Cargo.toml` descriptions and LOC counts, **not** from reading
  every line of all 25,567. Treat per-crate "why" lines as the author's stated intent until verified.
- The no-shell gate covers **file extensions**. It does not prove no crate shells out at runtime via
  `std::process::Command` — that is a separate, unbuilt check.

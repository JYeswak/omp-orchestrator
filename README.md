# omp-orchestrator — a native Rust binary that runs a project end to end

> Install it on any of our machines, point it at a repo, and it drives that project from a plan
> to shipped, verified work using OMP agents — planning → beads → PageRank triage → dispatch →
> ground-truth verification → close. No shell. No Python. One binary.

## What this is

A single installable Rust binary that owns the **whole project lifecycle**, not one slice of it:

```
  plan  ──►  beads  ──►  bv triage  ──►  OMP agent panes  ──►  verify  ──►  close
 (planning-  (beads-     (beads-bv,      (dispatch with       (ground     (typed
  workflow)   workflow)   PageRank)       typed acceptance)     truth)      receipt)
```

Today that pipeline exists as **160 shell scripts and 60,467 lines** in one repo, plus 20 Rust
crates that already ported the hard parts. This project is the consolidation: the Rust becomes the
product, the shell does not come with it.

## Why we are building it

**1. The shell is not incidental — it is the majority of the operational surface, and it keeps
producing defects that Rust makes inexpressible.** Measured in a single lane on 2026-08-31:

| Defect | What happened | Inexpressible in Rust because |
|---|---|---|
| Backtick injection | A backtick inside a *bead body* was executed as a command. `msg` was never assigned, so every scheduled fire dispatched **nothing** while still logging healthy idle counts. | Bead text is a `String`, never a program |
| `mapfile` absent | cron resolves `env bash` to **bash 3.2**, where `mapfile` does not exist. Every hand-run test passed; the scheduled lane was dead. | One compiled binary, one runtime |
| Heredoc stole stdin | `cmd \| python3 - "$n" <<'PY'` — the heredoc claimed stdin so the piped JSON never arrived. Reported `idle=1 dispatched=0` **with no error**. | Typed function call, no stdin contention |

None of these were visible by reading the code. All three were found by watching a scheduled lane
fail silently.

**2. A lifecycle spread across 176 scripts cannot be installed anywhere else.** The value is the
*process* — plan → beads → triage → dispatch → verify — and a process you cannot install is a
process that lives in exactly one checkout on one machine.

**3. Typed state makes the honest answer cheap.** "Is this pane working?" and "did this bead
actually close?" are enum questions. In shell they are string-matching questions, and every string
match we shipped has been wrong at least once.

## Scope boundary

**control-plane is the proving ground.** This substrate travels to another repo *only when proven
there*. Building it does not deploy it. Every claim below is measured in control-plane or it is
marked as unproven.

## Non-goals

- **Not** rebuilding NTM, FrankenTerm, Agent Mail, `br`, or `bv`. We wrap the existing stack.
- **Not** a resident daemon or ringleader agent. Stateless: artifacts and tools, human on a cadence.
- **Not** a shell-to-Rust transliteration. Where the shell was wrong, the port fixes it and says so.

## The cancellation contract (asupersync)

Every process this binary spawns — `tmux`, `ntm`, `br`, `bv`, a build — is cancellable work with a
deadline. It is built on **asupersync 0.4.9** (`/Volumes/ZestData/dicklesworthstone-mirror/asupersync`,
a real `[lib]`), and these are contract, not style:

- **`&Cx` first** in every async API we own; `cx.checkpoint()` in loops, retry bodies, and long
  handlers so cancellation is observable rather than hoped for.
- **Region-owned tasks** via `Cx::spawn` / `Scope` child regions. **No detached tasks** — a detached
  spawn is how a cancelled dispatch keeps writing to a pane that has moved on.
- **Kill the process GROUP, never the pid.** Measured: `child.kill()` signalled one pid while its
  grandchildren survived as orphans (`ppid=1`, 0.0% CPU) **still holding the admission lock**. Every
  timeout then guaranteed the next attempt failed too — the failure created the condition for its
  own repetition.
- **Drain the pipes.** Piping stdout *and* stderr then polling `try_wait()` deadlocks any child that
  writes past ~64 KiB (each stream has its own buffer). Measured: a `git log` that takes 0.9s from a
  shell sat at **0.0% CPU for 104s** as a child. The tell is 0% CPU with no children — a slow
  computation burns CPU, a deadlock does not, so widening the timeout makes it *worse*.
- **A timeout is not a verdict.** A killed child's empty stdout must map to `TIMEOUT`, never to the
  same token a genuinely failing subject produces. Measured: parsing an empty buffer for a `verdict`
  field and defaulting to `FAIL` manufactured a claim about the fleet out of nothing.

## The lifecycle, and which skill governs each stage

| Stage | Governing skill | What the binary does |
|---|---|---|
| Plan | `/planning-workflow` | Converge in plan-space first; plan-space is cheap, implementation-space is ~25× |
| Plan → beads | `/beads-workflow` | Every bead self-contained, with **testable acceptance: run X, expect Y** |
| Triage | `/beads-bv` | PageRank over the DAG; work the articulation points, not the comfortable leaves |
| Dispatch | `/vibing-with-ntm` | One bead, exact owned files, required proof, stop conditions — to a pane proven idle |
| Verify | `/beads-compliance-and-completion-verification` | **Re-run, don't read.** Status is a claim, not a fact |
| Close | typed receipt | Ground truth only: a commit, a bead close with cited evidence, a structured ack |

**The rule that binds all six:** a bead with no acceptance criteria cannot be worked, only
adjudicated — and adjudication reliably produces "no work to be done" instead of work. Measured:
a P0 bead at the head of the ready queue had **no ACCEPTANCE section at all**, and two agents in a
row triaged it and idled rather than shipping.

## Hard gates (they fail the build, not a report)

1. **No `.sh`, no `.py`.** A Rust gate walks `git ls-files` and refuses either extension. It lands
   *before* the first crate is copied — a gate that arrives after the mess gets weakened to make the
   build pass. Planted known-bad both directions plus a mutation leg.
2. **`#![forbid(unsafe_code)]` in every crate.** Today **2 of 20** carry it. A crate that will not
   compile under the lint is a finding, not a reason to drop the lint.
3. **Every gate proven to bite.** A gate with no fires-on-known-bad is not evidence of anything; a
   gate with *only* attack legs is over-strict and gets routed around. Both directions, always, plus
   a mutation that turns the leg RED.
4. **Anti-vacuity.** An empty scan set is an **error**, never a pass. A deliverable that was never
   checked reports identically to one that passed.

## Status

Pre-extraction. 20 crates / 25,567 LOC / 18 with tests identified in `control-plane/crates/`.
Nothing here is installed anywhere yet, and this README describes intent plus measured evidence —
not a shipped capability.

# PLAN.md — omp-orchestrator

**Status: ROUND 1 DRAFT.** Not converged. Not yet reviewed. Not yet materialized into beads.
Written 2026-08-31 after a stand-down called because the project had been running without one.

> **The one sentence.** *A single installable Rust binary that takes a repository from an idea to
> shipped, verified work by driving a fleet of AI coding agents — where every stage is typed,
> every claim is backed by a re-runnable command, and the machine refuses to report success it
> cannot prove.*

---

## 0. How to read this document, and how to attack it

This plan is written to be **failed**. If you are reviewing it, your job is to find the claim that
does not survive contact with evidence. Three conventions make that possible:

- **Every number carries the command that produces it.** If you cannot re-derive it, it is a bug in
  this document, not a fact about the project.
- **`NO-CLAIM` paragraphs are load-bearing.** They mark the boundary of what the adjacent claim
  covers. A section without one is either trivially true or under-examined.
- **`MEASURED` means observed on this machine on the stated date. `PROJECTED` means reasoned.**
  They are never mixed in the same sentence.

The most valuable review finds a `MEASURED` that is actually `PROJECTED`.

---

## 1. The problem, measured

A multi-agent coding fleet produces work faster than any human can verify it. The failure is not
that agents write bad code — it is that **nothing in the loop can tell a finished thing from a
thing that merely looks finished**.

This is not a hypothesis. In a single session on 2026-08-31, on this repository, with five agents:

| failure | measured |
|---|---|
| A mechanism built, tested, hardened, and **called by nothing** | **20 occurrences** |
| Crates with no path to execution | **12 of 20** at time of census |
| A supervisor emitting a typed refusal that no one read | **162 consecutive ticks** |
| An idle-capacity alarm written to a file with one writer and zero readers | **178 consecutive ticks** |
| Fleet idle while every watchdog reported healthy | **4h 19m** |
| Installed binary behind source | **23 commits** |
| Dispatches performed by a human because the actuator did not exist | **~30** |

Every one of those was found by a person noticing. **None was caught by the system.**

The economic shape: agent time is cheap and getting cheaper; **verification is the scarce
resource**. A fleet that cannot verify itself converts cheap agent-hours into expensive human
review-hours at a fixed ratio, and that ratio is the ceiling on how many agents one person can run.

**NO-CLAIM.** These figures are from one repository, one operator, one day. They establish that the
failure mode is *real and frequent here*. They do not establish a market-wide rate, and this plan
makes no claim about how other teams' fleets behave.

---

## 2. What we are building

A single binary, `omp-orchestrator`, that owns the whole lifecycle rather than one slice:

```
plan ──► beads ──► triage ──► dispatch ──► observe ──► verify ──► close
```

Each arrow is a **typed transition** with a refusal path. The binary cannot report a healthy fleet
while a gate is unwired, cannot dispatch into a pane it has not confirmed idle, and cannot close a
bead without cited, re-runnable evidence.

### The three properties that distinguish it

**1. Refusal is a first-class outcome.** Most orchestrators answer *"did it work?"* with a boolean.
This one answers with an enum whose arms include `GateUnwired`, `MonitorBlind`, `QueueUnreadable`,
and `EscalateIdleIncident`. There is deliberately **no arm meaning "nothing to do"** when
dispatchable panes and ready work coexist.

**2. A timeout is not a verdict.** A killed child's empty output maps to `Cancelled`, never to the
token a genuinely failing subject produces. This is enforced at the type level by
`asupersync::Outcome<T,E>` — `Ok | Err | Cancelled(CancelReason) | Panicked(PanicPayload)` — where
cancellation cannot be confused with error because they are different variants.

**3. Every gate proves it bites.** A gate ships with a planted known-bad, a mandatory known-good,
and a mutation leg that turns the known-bad RED. A gate with only attack legs is over-strict, gets
routed around, and dies slower than no gate at all.

### What it is not

- Not a rewrite of NTM, Agent Mail, `br`, or `bv`. It wraps them.
- Not an autonomous ringleader. Idle capacity requires dispatch, a typed escalation, or an
  operator's **expiring** authorization.
- Not a shell-to-Rust transliteration. Where the shell was wrong, the port fixes it and says so.

---

## 3. Who this is for, and the three workflows that matter

**Persona: the operator.** One engineer running four to six agents on a real codebase, who cannot
read every diff and needs the system to tell them where to look.

### Workflow A — "I have an idea and want it shipped"

*Trigger:* operator writes a plan. *Steps:* plan converges → materialize the full bead DAG →
`bv` ranks by PageRank → binary dispatches to confirmed-idle panes → workers land commits → binary
verifies by re-running cited commands → beads close with evidence. *Outcome:* shipped work the
operator did not have to babysit.

**What breaks today:** the actuator is a human. Measured ~30 hand dispatches in one session.

### Workflow B — "something is wrong and I need to know what"

*Trigger:* a metric moves or a pane goes quiet. *Steps:* one robot command answers *what is true
right now* — pane states, queue depth, gate reachability, drift between source and installed —
with a versioned JSON envelope and no prose to parse. *Outcome:* the operator reads one output
instead of five terminals.

**What breaks today:** the observer finds idle panes and discards them one layer up
(`free_capacity: []` beside `state=IDLE`).

### Workflow C — "prove this is actually done"

*Trigger:* a worker reports completion. *Steps:* the binary re-runs the cited command, compares
against the claim, and either closes with the transcript or refuses with the discrepancy named.
*Outcome:* a close is evidence, not a status.

**What breaks today:** grading is prose written by a human. Measured: 9 beads closed on re-run
evidence in one session, **2 refused after their author reported success**.

---

## 4. Architecture

### 4.1 The decision function is the product

```
decide(observation, authorization) -> SupervisorDecision
  1. GATE CENSUS   ─► GateUnwired            unreachable-around, checked FIRST
  2. panes empty   ─► MonitorBlind
  3. queue unread  ─► QueueUnreadable
  4. no capacity   ─► SupervisedWorking      heartbeat, not an alarm
  5. free + ready  ─► Dispatch | EscalateIdleIncident | AuthorizedIdle
```

Pure, exhaustively matched, no wildcard arms — so adding a variant is a **compile error** in every
consumer rather than a silent fallthrough. That is not a style preference: when `GateUnwired` was
added, a consumer failed to compile with `E0004`, which is the design working.

### 4.2 The four layers, and which exist

| layer | mechanism | status |
|---|---|---|
| observe | `tick-monitor` — pane state from ground truth | **works** |
| actionable | `idle_panes` — capacity for a dispatcher to consume | **broken**: discards NewlyIdle |
| consume | `decide()` in the resident supervisor | **fenced** — 162 refused ticks awaiting an operator decision |
| actuate | dispatch to a pane | **does not exist** — a human does it |
| complete | worker says done | **does not exist** — inferred, after a deadline |

**This table is the plan.** Everything in §7 exists to turn each row green with evidence.

### 4.3 Why asupersync, and what we get that we would otherwise build

Every subprocess this binary spawns — `tmux`, `ntm`, `br`, `bv`, a build — is cancellable work with
a deadline. The contract is not style:

- **`&Cx` first**, `cx.checkpoint()` in loops, region-owned tasks, **no detached spawns**.
- **Kill the process GROUP, never the pid.** *Measured:* orphaned grandchildren at `ppid=1`, 0.0%
  CPU, still holding the admission lock — so every timeout guaranteed the next attempt failed too.
  The failure created the condition for its own repetition.
- **Drain both pipes.** Piping stdout *and* stderr then polling `try_wait()` deadlocks past ~64 KiB.
  *Measured:* a `git log` taking 0.9s from a shell sat at **0.0% CPU for 104s** as a child. The tell
  is 0% CPU with no children — so **widening the timeout makes it worse**.

It also already contains vocabulary we were reinventing: `AckKind` (the acknowledgement boundary an
operation reached), `DeliveryClass` (five semantic weights, each mapped to its minimum ack),
`PublishPermit` (`#[must_use]`, aborts cleanly on drop with no obligation leaked), and
`ObligationLedger` (reserve, then commit or abort).

**NO-CLAIM, and it is a live blocker.** `AckKind` and `DeliveryClass` sit behind
`feature = "messaging-fabric"`, which **does not compile at our pinned rev** `fa3c01aec`:
`messaging/consumer.rs:1299` has an un-gated `fn default()` calling `TaskId::new_ephemeral()`, which
is `#[cfg(any(test, feature = "test-internals"))]`. Measured both ways — `messaging-fabric` alone →
`E0599`; plus `test-internals` → exit 0. Enabling the test feature would reintroduce exactly the
production leak upstream issue #46 closed. **The ack vocabulary is unavailable to us today.**

---

## 5. The gates — how the system refuses

A gate is not a linter. It is a mechanism that makes a class of defect **unrepresentable or
unshippable**, and it must satisfy five properties:

1. **Fires on known-bad**, with the specimen **in-tree** — an external patch harness silently no-ops
   when its index hash misses HEAD.
2. **Passes known-good.** Mandatory. Without it a gate could refuse everything and look identical
   to one that works.
3. **Mutation turns the known-bad RED**, restored byte-identically with the sha reported both sides.
4. **Anti-vacuity: an empty scan set is an ERROR**, never a pass. A deliverable never checked
   reports identically to one that passed.
5. **Reachable trigger on the machine it runs on.** A gate wired to CI in a repo with no remote is
   a gate that has never fired.

### Current gate inventory — MEASURED 2026-08-31

| gate | enforces | trigger | state |
|---|---|---|---|
| unwired-lane conformance | every lane has a production caller | `cargo test` | **RED on `installer`** — caught a real lane unprompted |
| no-shell — no `.sh`/`.py` | language boundary | `.git/hooks/pre-commit` | green, proven to refuse a staged `.sh` at exit 1 |
| commit-msg round-trip | message byte-identical to a staged file | `.git/hooks/commit-msg` | green — refused the conductor's own commit |
| kernel-bypass | handrolled equivalents of existing kernels | none | **built, not installed** |
| pre-delete citation | deleting a path a closed bead cites | none | **built, not installed** — has fired twice by hand |
| state-wildcard lint | wildcard arms on state enums | none | **built, not installed** |
| undrained-pipe lint | `try_wait()` without draining | none | **built, not installed** — 29 candidate sites unscanned |

**Three of seven have a reachable trigger.** That ratio is the single most honest number in this
document.

**NO-CLAIM.** A gate proves shape, never behaviour. A crate can satisfy every gate and still leak a
detached task, kill a pid instead of a process group, or map a timeout to the token a failing
subject produces. Those three cost real time and **none of them is greppable**.

---

## 6. Crates, types, and schema

### 6.1 Inventory — MEASURED

25 crates. `cargo metadata --no-deps` reverse edges, never grep — a census that greps crate names
matches its own table.

| axis | measured |
|---|---|
| forbid(unsafe_code) | 16 of 22 at census |
| depend on asupersync | 6 of 22 — the cancellation contract covers **27%** of the workspace |
| `async fn` taking `&Cx` first | 12 of 14 |
| raw `Command::new` sites | **29**, against **4** crates using the drain-safe runner |
| forbidden deps (tokio/hyper/axum/…) | **0** |

### 6.2 Types — MEASURED

51 public enums, 79 structs, 22 of 24 crates. Four findings:

- **Four colliding names**, one structural: `tick-monitor` produces the `Observation` that
  `omp-orchestrator` consumes and **each declares its own incompatible struct**. No shared type
  crosses the seam, so producer and consumer agree only by convention — which is exactly where
  `free_capacity` was derived from the wrong filter.
- **Six independent `Verdict` types**, none composable, none countable. This is *why* grading is
  prose: there is no type a grade can be.
- **Seventeen ack/receipt types in three dialects** — `Authority`, `Receipt`, and a third.
- **Missing**: `Grade`, and four types asupersync already has.

### 6.3 The declaration schema

Three per-crate declarations exist. **Three of four are read by nothing** — `omp-types` has zero
dependents, `ASUPERSYNC-CONFORMANCE.md` has zero consumers. §7 turns each into a test leg.

---

## 7. Milestones — and what *done* means at each

Done is never "the code exists." Each milestone below states the **observable** that closes it.

### M1 — The gate floor
*Done when:* all seven gates have a reachable trigger on this machine, and the five-leg conformance
test refuses an unconformant crate on its own in-tree specimen.
*Observable:* `cargo test` fails on a planted violation of each leg, one leg at a time.
*Blocked by:* nothing.

### M2 — Source ≡ installed ≡ running
*Done when:* one command installs from clean and proves four-way identity, and a standing drift
check exits nonzero when they diverge.
*Observable:* `installer --check` exits 0 after a real install; exits 1 with both identities named
when a commit lands.
*Currently:* the check works and exits 1 on real drift; the install path's known-good is blocked on
a container floor.

### M3 — The loop closes without a human
*Done when:* one full tick runs `observe → decide → dispatch → receipt → close` with no operator
action, and the transcript shows all three ack authorities captured separately.
*Observable:* a bead moves `open → in_progress → closed` with cited evidence, and the operator
learns of it from a completion signal rather than by looking.
*Blocked by:* M1, M2, the `free_capacity` defect, and the completion signal.

### M4 — Installable elsewhere
*Done when:* a second machine runs the loop from a clean clone.
*Observable:* on a machine that has never seen this repo, install → `--check` green → one dispatch.
*Note:* `.git/hooks` is per-clone and untracked, so a fresh checkout has **no gates** — M4 is
strictly harder than it looks and M1 does not imply it.

### M5 — The substrate travels
*Done when:* the loop drives a repository that is not this one.
*Observable:* a bead closes in a target repo with evidence, driven by the same binary.

**NO-CLAIM.** M1–M3 are scoped to this machine. M4 is where most "it works" claims die, and this
plan does not treat M3 as evidence for M4.

---

## 8. What would make this fail

An investor-grade plan names its own kill conditions.

1. **The verification loop is slower than the human it replaces.** If proving a bead closed costs
   more than reading the diff, the product is a tax. *Mitigation:* every verification is a
   re-runnable command, not a review.
2. **Gates get routed around.** An over-strict gate is worse than none — it trains the operator to
   `--no-verify`. *Mitigation:* mandatory known-good leg on every gate. *Residual risk:* real, and
   only visible in usage.
3. **The scraping boundary never closes.** OMP's `--mode=rpc` is **single-session and cannot address
   a third-party pane** — measured. So cross-pane dispatch and delivery receipts may be permanently
   observational. *Consequence:* some defects stay detectable-but-not-preventable.
4. **The substrate is one person's workflow.** Everything here is measured on one operator's fleet.
   *Mitigation:* M4/M5 exist precisely to falsify this, and they are the milestones most likely to
   fail.
5. **Upstream drift.** We pin `asupersync` at a rev whose `messaging-fabric` feature does not build.
   *Consequence:* a vocabulary we depend on architecturally is unavailable operationally.

---

## 9. What is proven, and what is not

| claim | status |
|---|---|
| A conformance test can catch an unwired lane automatically | **BANKED** — fired on `installer` unprompted |
| A pre-commit gate can refuse a language violation | **BANKED** — refused a staged `.sh` at exit 1 |
| A commit-message gate can refuse its own author | **BANKED** — refused the conductor's commit |
| Three-authority dispatch receipt is capturable | **BANKED** — captured live, separately, once |
| Identity drift is detectable | **BANKED** — `installer --check` exits 1 naming both identities |
| The loop can close without a human | **UNPROVEN** — never happened |
| The substrate installs elsewhere | **UNPROVEN** — never attempted |
| Verification is cheaper than review | **UNPROVEN** — no measurement exists |

---

## 10. The reap — what the stand-down surfaced

The stand-down asked every worker for beads held, evidence, and **anything not in a bead**. That
last question produced more than the first two.

### 10.1 Landed and awaiting grade — 6

`0hk` `e9a410a` · `79am1` `2b18272`+`0f7134e` · `w4j` `b23591a` · `ilt` `9a61acd` ·
`ipg.13` `3760589` · `232` `449b20e`

**Zero landed-and-closed on the day they landed.** Work finishes and sits. That is the closure
debt the completion signal (§4.2, row 5) exists to remove, and it is measurable: 25 beads
`in_progress` against 28 closed across the whole project.

### 10.2 Not in any bead — 7, and this is the valuable half

| item | why it matters |
|---|---|
| `Observation` migration has **no bead** | the structural collision of §6.2 — the highest-value single row in the type inventory — was never work anyone was assigned |
| agentmail CLI quarantined | a coordination channel was removed from service and nothing records it |
| a self-reported claim-vs-content defect in `9abd64e` | the author found their own overstatement and it lived only in chat |
| **2 pre-existing red tests** in `no-shell-gate` | red that predates today's work, invisible because the suite was already expected to be red |
| hook install state | the gap between committed and installed, which bit three separate gates |
| an `E0433` refinement | a compile error routed around rather than fixed |
| RCH workers dark | the remote build fleet is unavailable, which is why the container floor blocks `M2` |

**Seven real conditions, none of them tracked.** They existed only in pane scrollback, which dies
with the pane. This is the concrete argument for §7's completion signal carrying *findings*, not
just a verdict.

### 10.3 A grade that refines rather than refutes

`ipg.17` (type inventory) was self-closed claiming *"gate refuses Observation seam with CONVERGE
decision."* Verified: the implementation is **real** — `types_inventory.rs:176-178` deliberately
excludes `Observation` from the allowance list and treats the collision as requiring convergence —
and 13 tests pass.

But the running binary tells a different story. `omp-inventory-map` with no arguments emits a
versioned JSON envelope (`command: doctor`, `status: UNKNOWN`, **exit 2**) whose 544 KB of output
contains **zero** occurrences of `Observation`, `CONVERGE`, or `Verdict`. And `--help` returns
`CONFIG_ERROR unknown argument --help`.

So the gate is **built and correct and undiscoverable**: an agent cannot find the subcommand that
runs it, because the binary has no help surface. Per `/agent-ergonomics`, one command must answer
*what is true right now* — and here the default command answers a different question while the
right one is unreachable without reading the source.

**This is the §5 gate-inventory problem at a finer grain.** It is not built-vs-wired; it is
wired-but-unaddressable. The counts it *does* emit — 39 CLI commands, 57 type roots, 42 RPC
handlers — independently reproduce measurements taken separately, so the scanner is sound.

**NO-CLAIM.** I verified the seam logic **in source and by test count**, not by executing the
subcommand that applies it — because I could not determine what that subcommand is. The claim is
substantiated, not independently reproduced.

### 10.4 What the reap changes about this plan

Three things, each of which alters a section above:

1. **§5's gate table understates the problem.** A gate can have a reachable trigger and still be
   unusable if no one can find how to invoke it. Add a sixth gate property: **addressable** — one
   documented command runs it, and `--help` names that command.
2. **§7's completion signal must carry findings, not just a verdict.** Seven untracked conditions
   surfaced only because a human asked. A `Finished{verdict}` with no findings channel loses them.
3. **§9 needs a row it does not have:** *"red that predates the current work is visible."* Two
   pre-existing failures hid inside a suite that was expected to be red for a different reason.
## 11. Open questions this plan does not answer

1. What is the extraction denominator? Stated as both 20 and 23; neither derived. 53 control-plane
   crates are candidates, some of which should be **retired rather than moved**.
2. Is `Observation` one type or two? Requires a decision, not an allowance row.
3. Can a completion signal survive pane death without a tracker round-trip?
4. Does NTM itself speak an OMP protocol beneath `--robot-send`? **Unmeasured** — evidence leans
   against, and leaning is not measuring.
5. What is the actual cost per verified bead? No instrumentation exists.

---

## Appendix A — review log

| round | reviewer | date | outcome |
|---|---|---|---|
| 1 | — | 2026-08-31 | initial draft, unreviewed |

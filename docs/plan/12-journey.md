# 12 — The dispatchable journey: a runbook per stage

> **Josh, 2026-08-31, defining the mission:** *"orchestrate 1 to 1 and 1 to many ntm sessions"*
> through *"a to z from project inception (new project, claude.md and agents.md, proper gates,
> proper infra), through to planning, grading, beads dag, execution, grading, validation, ship
> with all human requirements stored"* — and *"at each layer of the journey we need to define what
> amazing looks like."*

This section exists because §11.8 recorded that the A-to-Z process **exists, is distributed across
twelve skills, and has never been assembled**. §11 gave the lifecycle a spine. This gives each
vertebra a runbook: what the stage is, what amazing looks like, what it refuses, and which of our
own skills already encode it.

## Why a runbook and not a description

A stage that cannot be dispatched is not a stage — it is a paragraph. Every runbook below must
answer one question an orchestrator asks at three in the morning: **"I have a free pane and this
stage is next; what exactly do I send it, and how will I know it worked?"**

Anything that does not survive that question is prose, and prose is what this project keeps
catching itself producing.

## The nine stages

| # | stage | the artifact it must leave behind |
|---|---|---|
| S1 | Inception | repo, `CLAUDE.md`, `AGENTS.md`, gates that bite, infra that runs |
| S2 | Planning | a plan whose every number carries the command that derives it |
| S3 | Grading the plan | two clean rounds, two lenses, per section |
| S4 | Beads DAG | self-contained beads with testable acceptance, no cycles |
| S5 | Execution | commits that map to beads, path-scoped, evidence-cited |
| S6 | Grading the work | independent re-derivation, never a worker's self-report |
| S7 | Validation | the thing runs, on another machine, unattended |
| S8 | Ship | installable, versioned, with a rollback |
| S9 | Human requirements stored | every human decision durable and retrievable |

S9 is not last — it is **cross-cutting**. Every stage generates human decisions and every stage
loses them. This session alone produced eleven that live only in pane scrollback.

## The runbook contract

**SCOPE — read before applying this.** This contract governs the **nine journey stages S1–S9**
below. It does **not** govern the twelve plan sections `00`–`11`, which are analysis documents
written before this contract existed and answer a different question.

That sentence is here because its absence cost a round. In round 10 the investor lens applied this
contract to `06-gates`, `07-installability` and `08-end-users` and filed the same BLOCKER three
times — *"a whole-file search found no Trigger, Dispatch packet, Amazing…"* — which is true, and
irrelevant, because those files were never meant to carry it. **Three of that round's seventeen
findings were manufactured by my own briefing**, which is a defect in the instruction and not in the
grader: a contract that does not state its scope will be applied to everything in reach.

Each stage below MUST carry all seven of these. A stage missing any one is not dispatchable.

```
### S<n> — <name>

**Trigger.**        What state of the world means this stage is next.
**Dispatch packet.** What an orchestrator actually sends a pane. Concrete, not a topic.
**Amazing.**        The bar. Specific enough to fail. Not "high quality".
**Adequate.**       What ships when amazing is not affordable, and what that costs later.
**Negative patterns.** Named failure shapes, each with the measurement that proved it real here.
**Skills.**         Which of our jsm skills cover this, and what each does NOT cover.
**Done signal.**    The artifact + the command that proves it, exit code and all.
```

`Amazing` and `Adequate` are both required. A runbook with only `Amazing` gets routed around at
2am, and a routed-around stage is worse than an honest lower bar.

`Negative patterns` MUST cite something measured in this repo or a named upstream source. An
invented failure mode is a guess wearing a warning's clothes, and §00 §3.5 records what a bare
number is worth here.

---

<!-- STAGES S1-S9 ARE FILLED BY THE WAVE. Each is owned by exactly one pane. -->

---

## What the duel found: six gaps this plan structurally could not see

Three agents across **two model families** — GPT-5.6-Luna (`%1413`) and GLM 5.3 (`%1408`, `%1409`)
— independently answered *"what is missing from this plan for the A-to-Z 1:1 and 1:many mission?"*
Each generated 20 candidates and winnowed to 6, without seeing the others' work.

**Four of six converged across both families.** Convergence under independent generation is the
strongest signal this method produces — these are not one model's hobbyhorse.

| gap | 1413 | 1408 | 1409 | status |
|---|:--:|:--:|:--:|---|
| Human-decision ledger (S9's missing mechanism) | #3 | #1 | #3,#4 | **unanimous** |
| Fleet / project namespace identity | #1 | #3 | #A | **unanimous** |
| S1 inception envelope — never run cold | #4 | #4 | #E | **unanimous** |
| Runbook contract is missing fields | — | #5 | #8,#D | two families |
| Append-only event spine / packet journal | #2 | — | #F | two families |
| Leases with expiry (workers, sessions, waves) | #5 | — | #2 | two families |

### The three that are already measured, not speculative

**S9 has no mechanism at all.** §12's own stage table promises *"every human decision durable and
retrievable"* and nothing anywhere implements it. Eleven of Josh's rulings tonight live only in
pane scrollback and die at the next compaction. `%1408`: *"every pane in this session was re-briefed
by hand."* That is not a prediction about month three; it is what this session cost.

**1:many is in the mission and almost nowhere in the artifacts.** Measured tonight: `state_path()`
returned one fixed path for eight live sessions and the directory was hardcoded to a single
session's name (`9356bd5`). `.beads/.write.lock` is a single lock. The mission sentence says
"1 to many"; the substrate says one.

**Unowned resources already leak, and the corpse is on this machine.**
`zeststream-cast-wave-20260825-1910` is **6 days old, unattached, holding two live panes with
`node` still running.** It is not alone — `cmm2` (2 days) and `franken-harvest` (8 days) are also
unattached. **Three of eight sessions are orphaned right now.** `%1409` cited this as the argument
for expiry-bearing leases, and it is the strongest evidence in the duel because nobody had to
imagine it.

### The eighth type root, and an honest downgrade

`%1408` found `dist/types/plan-mode/` — a whole family we never swept: `approved-plan.d.ts`,
`plan-handoff.d.ts`, `plan-protection.d.ts`, `model-transition.d.ts`, `plan-files.d.ts`, `state.d.ts`.
It argued S2/S3 are *"the same adoption bet that just paid on completion."*

**Verified, and the claim is overstated — by me, before the duel scored it.** The types are real
but thin:

```typescript
interface PlanApprovalDetails  { planFilePath: string; title: string; planExists: boolean }
interface ResolvedApprovedPlan { planFilePath: string; planContent: string; title: string }
```

That is a plan **reference** and an approval **flag**. Completion gave us a wire-proven event
carrying a terminal discriminator; plan-mode gives us a file path and a title. It removes any excuse
for inventing a third plan-approval format, and it does **not** supply the grading or convergence
protocol S3 actually needs — which this repo had to build from scratch as `CONVERGENCE.jsonl` and
`convergence.rs`.

**The pattern still holds: sweep before building.** The magnitude does not transfer between roots.

### NO-CLAIM

Three of four ideation files are folded here; `%1414` was still generating when this was written and
its ideas are not represented. The duel's cross-scoring phase (each family scoring the other's ideas
0–1000) was **not run** — Josh redirected to convergence rounds, so what is recorded above is
*independent convergence*, which is weaker than *adversarially survived*. No idea here has been
attacked by a model that wanted it dead.

---

## Embedding the AAR harness shape: what we already have and the two legs we are missing

Josh pointed at **`YuehHanChen/automated_alignment_researcher`** (Chen Yueh-Han, Jiaxin Wen, Jan
Hendrik Kirchner — Anthropic research), specifically `generic_aar/`, which is that harness stripped
to a task-agnostic template. Read before answering. It is directly applicable, and comparing it to
what this repo built tonight is unflattering in a useful way.

### The AAR contract

> *"The one requirement: your task needs at least one **hill-climbing** benchmark (the objective the
> AAR optimizes) and one **held-out** benchmark (a different distribution / a fresh set, to test that
> a fix generalizes rather than overfits). Optional **capability** benchmarks act as don't-regress
> gates."*

Three roles, one scoring rule:

| role | meaning | our analogue |
|---|---|---|
| `safety` | hill-climbing; the agent sees and optimizes it | `CONVERGENCE.jsonl` — **we have this** |
| `held_out` | different distribution, **stripped from the agent-facing result** | **MISSING** |
| `capability_filter` | a `floor` that must not regress | **MISSING** |

`closed% = (score − baseline) / (optimum − baseline)`, geometric mean over the hill-climbing legs,
gated by the capability filter, held-out scored eval-private.

### Hole 1 — every lens sees every section, so we cannot detect lens-adaptation

Our convergence rule is *two clean rounds under two different lenses*. It has no held-out leg, which
means a section can converge because **the graders adapted to each other** rather than because the
section is sound. Four lenses have now been over this plan repeatedly; they have read each other's
findings in the ledger; that is precisely the overfitting AAR's held-out leg exists to catch.

**The fix is cheap and it is a lens, not a section.** A held-out lens is a genuinely different
distribution over the same document. One lens is withheld from every round, then run across all
twelve sections at the end. A section that converged under two lenses and then fails the unseen one
did not converge — it was ground smooth against the graders it had met.

### Hole 2 — a converged section is frozen and nothing re-checks it

`03-crates`, `05-actions` and `06-gates` are CONVERGED as of round 9. Rounds 10 and 11 will edit
*other* sections — and several findings this session were cross-section (the `370`-vs-`379` count
propagated from `06-gates` into `01-idea`; the `AgentEndEvent` refutation had to be chased across
five files). **Nothing re-checks a converged section after a neighbour is edited.** That is exactly
what `capability_filter` with a `floor` prevents: you may not improve the thing you are grinding by
regressing something already banked.

### What this changes, concretely

`CONVERGENCE.jsonl` rows gain a `role`, and `convergence.rs` gains a floor check:

```jsonc
{"section":"06-gates","round":10,"lens":"absence","role":"capability","new_findings":0,…}
{"section":"04-diagrams","round":10,"lens":"investor","role":"hillclimb","new_findings":1,…}
{"section":"00-brief","round":12,"lens":"HELD_OUT","role":"held_out","new_findings":?,…}
```

- **hillclimb** — the section being worked this round.
- **capability** — a re-check of an already-converged section. Any finding **un-converges it**, and
  the count goes down. This is the floor.
- **held_out** — the withheld lens, run once at the end across everything.

### What does not transfer

AAR hill-climbs a **numeric benchmark score** with a normalized closed-fraction against a measured
baseline and a known optimum. Our signal is `new_findings`, an integer with **no optimum** — zero
findings is not "solved", it is "nobody found anything this round", and this session has produced
eleven false zeros that looked exactly like measurements. So the geometric-mean closed-% machinery
does not port; the **three roles and the isolation discipline** do.

The **integrity monitor** — a separate agent that approves proposed code before it runs — has no
analogue here and is the third thing worth stealing. Tonight I committed a vacuous gate
(`BASELINE=24` against a detector measuring 13) and caught it myself only because a mutation leg
happened to be part of the same commit. An approver that never wrote the code would have asked what
the number was measured with.

### NO-CLAIM

This section maps the AAR shape onto ours from its `README.md` and `generic_aar/README.md`. **The
harness has not been run here** — it targets Linux + CUDA and this is an Apple Silicon Mac, so even
the no-GPU stub path is untested by me. What is claimed is that the *role taxonomy* and the
*isolation discipline* are transplantable, and that we are measurably missing two of three roles.

---

## 12.10 The milestone loop — what runs before anything is built

> **Josh:** *"we establish — what happens foundationally for each stage first that everything else
> builds upon — gates, crates, input/output, schema, what needs to be true, negative patterns …
> we need to ensure we have all knowns, all unknowns, and gaps ahead of build."*

The seven-field runbook contract above says how to **dispatch** a stage. It does not say what must
be **true before the stage can be dispatched at all**, and it has no place to record what we do not
know. Two fields were missing, and they are the ones that run first.

### Field 8 — FOUNDATION (runs before any bead in the stage is created)

A stage cannot be worked until its substrate exists. Enumerate, in this order, because each is the
input to the next:

| # | foundation element | the question it answers | refusal if absent |
|---|---|---|---|
| F1 | **Schema** | what shape does this stage read and write | no `SCHEMAS.toml` row → the stage may not persist anything |
| F2 | **I/O contract** | who produces the input, who consumes the output | an unnamed consumer → the stage is BUILT ≠ WIRED by construction |
| F3 | **Crates** | which crate owns the mechanism, which is a thin caller | mechanism in a binary → untestable, and 21 of ours already are |
| F4 | **Gates** | what refuses a bad result, and does it bite | no known-bad leg → the gate is decorative |
| F5 | **Numbers** | which figures does this stage claim | undeclared figure → silent rot, measured 5 rounds running |

**Order is not stylistic.** A gate written before its schema gates a shape that will change; a crate
written before its I/O contract acquires the wrong seam. Every foundation inversion this repo has
suffered was one of those two.

### Field 9 — THE EPISTEMIC LEDGER (knowns, unknowns, gaps)

Three columns, kept per stage, and the third is the one that pays:

- **KNOWN** — measured, with the command. Goes in `NUMBERS.toml` if it is a figure.
- **UNKNOWN** — named, with *the experiment that would resolve it and its cost*. An unknown without
  a resolving experiment is a worry, not an unknown.
- **GAP** — we know the thing is missing and we know what it costs to leave it missing. A gap with
  no cost is a preference.

**Why this is a field and not a document.** The single most expensive discovery of this session was
that seven "gaps" had upstream types in the tool we wrap, and the eighth (`plan-mode`) turned up two
hours later. Every one had sat in prose as a settled absence. **An unknown that never had a resolving
experiment attached is indistinguishable from a known** — and §10 called one of them "precedent-free
across 210 repositories" while the precedent shipped in the binary named on line one.

### The loop, per milestone

```
  F1..F5 foundation      -> if any refuses, the stage is not dispatchable. Stop.
  epistemic ledger       -> every UNKNOWN gets a resolving experiment + cost
  cheapest falsifier     -> run the experiment that could kill the stage first
  beads                  -> WHAT / WHY / ACCEPTANCE, labelled, in the DAG, runnable
  dispatch               -> fresh eyes; the grader has never read the ledger
  grade + fix            -> same round, not the next one
  capability re-check    -> every previously-banked stage, or the bank is a fiction
```

**The cheapest falsifier runs before the beads exist.** Ordering independent checks by cost is
`beads-north-star`'s DAG rule, and it applies to the stage as a whole: the experiment that could
kill the milestone is worth more than the twenty beads that assume it survives.

### What is enforcing this today

Honestly: **three of nine fields, and the bead standard.**

| enforced | by | since |
|---|---|---|
| F1 schema | `schemas.rs` + `SCHEMAS.toml` | this session |
| F4 gates bite | `no-shell-gate` mutation legs | this session |
| F5 numbers | `numbers.rs` + `NUMBERS.toml`, 13 figures | this session |
| bead shape | `bead_standard.rs` — **plan-derived beads have no ratchet** | this session |
| F2, F3, field 9 | **nothing** | — |

`bead_standard.rs` splits the board in two: legacy beads get a ratchet from their measured floor
(4 of 50 met the full standard, 17 isolated, 54% with no runnable acceptance), and **plan-derived
beads are held to the whole standard from the first one**. It currently reports that zero exist —
which is exactly when a standard is cheapest to install, and is the difference between a gate and a
cleanup project.

**NO-CLAIM.** F2 and F3 have no mechanism; a stage can still declare an unconsumed output or put its
logic in a binary and nothing objects. The epistemic ledger has no schema, no gate, and no instance —
it is a specification for a field that does not yet exist anywhere in this plan.

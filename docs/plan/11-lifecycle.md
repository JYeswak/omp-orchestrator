# 11 — The lifecycle: idea to shipped, walked down the crates and the skills

**R13, added by Josh mid-grading:** *"part of our plan needs to be intimately aware of the entire
lifecycle of an idea to a finished project then walk the list down the crates and our skills to
ensure that throughout dispatch we have proper templates, proper dispatch, proper reap, proper
logging, proper build grading, etc."*

No section was written against R13, so this one is new and its findings are mostly **absences**.
That is the expected result and it is the point: the plan had ten sections describing *parts* and
none describing the *spine*. A reader could finish §00–§10 knowing every crate and still not know
what happens between "someone has an idea" and "it shipped."

Every claim here is `MEASURED` unless marked `PROJECTED`.

---

## 11.1 The nine stages, and who owns each

| # | stage | owning crate(s) | covering skill | measured state |
|---|---|---|---|---|
| S1 | **idea** | none | `/idea-wizard` | **NO CRATE.** Ideas arrive in chat and die there — the failure R11 was written to stop |
| S2 | **plan** | none | `/planning-workflow` | **NO CRATE.** This document is the artifact; nothing enforces its shape |
| S3 | **bead** | none (uses `br`) | `/beads-workflow` | wrapped, not owned. Close-policy lives in `br` |
| S4 | **select** | `loop-queue-filter` | `/beads-bv` | crate exists; **`Command::new("bv")` appears nowhere** — the graph is never called |
| S5 | **dispatch** | `dispatch-claim-fence`, `pane-dispatch-fence`, `fast-dispatch`, `tick-dispatch` | `/ntm-fleet-monitor` | four crates, one fence up 4.2h, **actuation is a human** |
| S6 | **work** | — | `/vibing-with-ntm` | the agent's own business; we observe only |
| S7 | **reap** | `reap-finished-panes` | `/vibing-with-ntm` | crate exists; the only reap this session was hand-run |
| S8 | **grade** | `verify-dispatch`, `ack-spine`, `ack-stage` | none | grading is prose; **6 Verdict types, no shared trait, nothing countable** |
| S9 | **ship** | `installer`, `commit-build-fence` | `/installer-workmanship` | installer covers 3 of 18 binaries; `--install` never run against a foreign host |

**Four of nine stages have no owning crate.** S1, S2, S6 arguably should not — an idea is not a
program. But **S3 has none and should**: the bead lifecycle is where our close-policy discipline
lives, and it is entirely delegated to `br`, which means our rules about evidence-bearing closes
are convention, not code.

---

## 11.2 The five dispatch properties Josh named, measured per stage

For each stage: is there a **template**, a **dispatch** mechanism, a **reap**, **logging**, and
**build grading**? `Y` = exists and was used this session. `y` = exists, unused. `—` = absent.

| stage | template | dispatch | reap | logging | build grading |
|---|:--:|:--:|:--:|:--:|:--:|
| S1 idea | — | — | — | — | — |
| S2 plan | — | — | — | — | — |
| S3 bead | — | n/a | — | `Y` (`.beads/issues.jsonl`) | — |
| S4 select | — | n/a | n/a | — | n/a |
| S5 dispatch | **`y`** | `Y` (hand-rolled) | n/a | partial | n/a |
| S6 work | n/a | n/a | n/a | — | n/a |
| S7 reap | — | n/a | `y` | — | n/a |
| S8 grade | — | n/a | n/a | `Y` (bead comments) | **`Y`** |
| S9 ship | — | — | n/a | — | `Y` |

**Reading the table honestly: 3 cells are `Y`-and-used out of 45. Two are `y`-exists-unused, and
those two are the most damning, because they cost nothing to adopt.**

---

## 11.3 The template we own and did not use

`ntm template list` returns four templates. One is exactly the thing this session hand-rolled
roughly thirty times:

```
Name:        dispatch
Description: ZestStream controller dispatch packet — bounded assignment with proof obligations
             and a named …
Path:        /Users/josh/.config/ntm/templates/dispatch.md
Variables:
  - objective (required)   ONE outcome, stated as a result not an activity
  - target    (required)   Absolute repo/worktree path, and the bead ID
  - why_now
```

Every dispatch packet in this session was written by hand into a `/tmp` file and sent with
`tmux send-keys -l`. The template has **required variables that fail closed on omission** —
`objective`, `target` — and `ntm send -t dispatch --var … --dry-run` performs full substitution and
refuses on a missing required variable.

**Two dispatch defects this session would have been structurally impossible with it.** The
`5rh`-to-`%1413` packet named an **unclaimed bead** — the missing file → CLAIM → dispatch middle
beat — and `target` is a required variable pairing the path *with the bead ID*, which is where a
claim check belongs. And the `omp-coverage-mission-ipg.4` packet was sent without the claim written
first, for the same reason.

That is the sharpest instance of `BUILT ≠ WIRED` in the lifecycle: not a mechanism nobody built,
but **a mechanism already built, already correct, and never invoked by the person who most needed
it.** `NO-CLAIM:` I have not verified that the template's body would have caught these two cases —
only that its required-variable contract addresses the field that was wrong in both.

---

## 11.4 The build-grading hook is a shell script, and the rule cannot see it

The repo's one hard rule is **no `.sh`, no `.py`**, enforced by `no-shell-gate` with an empty
exemption list. Measured:

```
ls -la .git/hooks/*.sh
  .git/hooks/commit-msg-verification-level.sh   6288 bytes

git ls-files | grep -c commit-msg-verification
  0
```

**The script that enforces our build-grading discipline is a 6.3 KB shell script**, and it is
invisible to the rule because `no-shell-gate` scans the git index. The gate is not defective — it
declares this boundary itself, at `crates/no-shell-gate/src/lib.rs:14`: *"this gate covers FILE
EXTENSIONS of tracked files, nothing else."* An honest, stated limit.

The finding is about **coverage, not correctness**: our most consequential policy hook lives in the
one directory the policy cannot reach. Three readings, and the plan must pick one:

1. `.git/hooks` is legitimately outside the rule — hooks are machine-local, never distributed, and
   a shell hook is the ecosystem norm. Then say so in the rule, because right now the rule reads
   absolute and is not.
2. It is a real violation and the hook should be a Rust binary like the other five gates. Note the
   installed `pre-commit` already *is* Mach-O, so precedent favours this.
3. It is a violation we accept with a named reason and an owner — the allowance-row shape.

**Silence is the one unacceptable option**, and silence is the current state. Registered as **Q13**.

---

## 11.5 Where the lifecycle actually breaks

Reading §11.1 and §11.2 together, the spine has three severed links, and they are not the ones the
crate list would suggest:

**S4 → S5, selection to dispatch.** `loop-queue-filter` exists and `bv` provides the graph, but
`grep -rn 'Command::new("bv")'` over `crates/` returns **no matches** — measured. Nothing in this
workspace ever invokes the ranking tool. Selection this session was the conductor picking by
recency of his own discovery, which `/beads-bv` names as the cherry-picking pathology, and the
graph disagreed: the top-3 PageRank items sat unclaimed.

**S6 → S7, work to reap.** There is no worker→conductor completion path *wired*. **The type exists** — `AgentEndEvent` at `dist/types/extensibility/shared-events.d.ts:154`, carrying `willContinue` to separate a terminal settle from a scheduled continuation, which is the exact `NewlyIdle`/`ConfirmedIdle` distinction our own filter gets wrong. The gap is adoption, not invention. §10 Gap 7 measured
that this is precedent-free across 210 mirror work-trees — **since REFUTED: OMP itself ships `AgentEndEvent`; the mirror search never searched the binary we wrap**. Original text: — `SupervisionEvent` has 8 variants and
`StopReason` 6, and not one of the 14 means *"the worker finished."* Every completion tonight was
found by a human looking.

**S8, grade, has no template and no type.** Grading produced four long prose documents this
session. `omp-types` exists to fix exactly this and has **zero dependents**, and the half of the
vocabulary that would collapse the ack dialects is blocked upstream behind
`messaging-fabric → test-internals` (issue #46). So the type that would make a grade *countable*
is not merely unadopted — it is unreachable at our pinned rev.

---

## 11.6 What R13 demands that is not yet built

| # | demand | state |
|---|---|---|
| L1 | every dispatch goes through the `dispatch` template, not a hand-rolled file | **not adopted** — template exists, unused |
| L2 | selection calls the graph | **not wired** — zero `bv` invocations in the workspace |
| L3 | a worker can signal completion in a typed way | **precedent-free**; the largest gap in the plan |
| L4 | a grade is a value, not a document | **blocked upstream** at the feature boundary |
| L5 | reap runs before refill, mechanically | **hand-run once**; `reap-finished-panes` exists uncalled |
| L6 | every stage writes a durable record | **3 of 9 stages log anything** |
| L7 | the build-grading hook obeys the repo's own rule, or names its exemption | **unresolved**, Q13 |

---

**NO-CLAIM.** This section maps nine stages and measures five properties across them. It does not
establish that nine is the right decomposition — S1/S2/S6 may not want crates at all, and a
different cut might make the severed links appear elsewhere. It does not measure how often each
break actually costs anything; the `bv` gap is measured as *never invoked*, not as *decisions made
worse*, and those are different claims. Every `PROJECTED` remedy in §11.6 is unbuilt and unowned
except where an owner is named.

---

## 11.7 The surface mapping converged on one crate

R14/R15 batches 1–9 mapped **270 of 544 surfaces** across `ntm`, `br`, `bv` and OMP. Dispositions:

```
RETIRE    243     CONSUMED  8     WIRE  11     VALIDATE  8
```

**243 of 270 retire, and that is the correct result** — adopting a surface because it exists is the
opposite of this plan's discipline. The value is in the 11.

### Six of eleven WIREs name the same crate

| surface | → crate |
|---|---|
| `bv:robot` | `loop-queue-filter` |
| `bv:candidates` | `loop-queue-filter` |
| `bv:decision-relevant` | `loop-queue-filter` |
| `bv:dependencies` | `loop-queue-filter` |
| `bv:not-ready` | `loop-queue-filter` |
| `br:blocked` | `loop-queue-filter` |
| `br:dep` | `loop-queue-filter` |
| `ntm:template` | `omp-orchestrator` |
| `ntm:version`, `br:version` | `installer` |
| `cli_command:usage` | `tick-monitor` |

Three agents working disjoint batches, given no shared hypothesis, independently routed **seven
selection-related surfaces into `loop-queue-filter`** — the crate that owns lifecycle stage **S4**,
the stage §11.5 names as severed, in the tool (`bv`) measured at zero consumption.

That convergence is worth more than any single mapping. It was not seeded: the batch packet named
four dispositions and no crate, and §11.5's S4 finding was not quoted to the mappers.

**`loop-queue-filter` is the missing consumer of the graph, and now seven surfaces say so.**

### The eight VALIDATEs are almost all `br`

`br:close`, `br:create`, `br:init`, `br:list`, `br:schema`, `br:sync`, `br:update` — we depend on
`br`'s **behaviour** without asserting on it anywhere. This is §11.1's S3 row measured from the
other direction: the close policy that refuses prose, the schema our beads must satisfy, the sync
that keeps `.beads/issues.jsonl` honest — all inherited, none tested. If `br` changed its close
policy tomorrow, nothing in this workspace would fail.

`bv:exit-codes` is the eighth, and it belongs with them: we would be reading `bv`'s exit contract
without a test pinning it.

**NO-CLAIM.** 270 of 544 rows are mapped; the remaining 274 are OMP, `ee` and `ms` and may shift
these proportions. A `WIRE` disposition is a *proposal with a named beneficiary*, not a decision —
none of the eleven has been implemented, and the convergence on `loop-queue-filter` argues the crate
is the right home, not that wiring it is scheduled or scoped. The `bv` rows carry the §7.2 scrape
defect: they are subcommand-shaped names harvested from help prose, so `bv:robot` stands in for the
40+ real `--robot-*` flags rather than naming one.

---

## 11.8 The A-to-Z process exists, is distributed across twelve skills, and has never been assembled

Josh: *"what is the defined — from a to z — typed process for managing an idea to shipped project
with swarm orchestration? use jsm search and lets identify this."*

Answer: **there is no single defined process. There are twelve skills that each own one stage, and
nothing composes them.** Found via `jsm search`:

| stage | skill(s) | typed? |
|---|---|---|
| S1 idea | `idea-wizard`, `dueling-idea-wizards`, `brainstorming` | **no** — prose in, prose out |
| S1.5 viability | `product-viability-gauntlet` | **partly** — fail-closed kill/narrow/pilot/build verdict |
| S2 plan | `planning-workflow` | **no** — markdown; convergence judged by eye |
| S2.5 loop | `loop-engineering` | **partly** — tick-loop with a verified-value bar |
| S3 bead | `beads-workflow`, `beads-north-star`, `beads-br` | **yes** — `br` schema, typed close policy |
| S4 select | `beads-bv` | **yes** — PageRank over a typed DAG |
| S5 dispatch | `ntm`, `vibing-with-ntm` | **partly** — `--robot-*` JSON, no receipt type |
| S6 work | `vibing-with-ntm` | **no** |
| S7 reap | `vibing-with-ntm` | **no** |
| S8 grade | `beads-compliance-and-completion-verification` | **no** — prose verdicts |
| S8.5 honesty | `just-say-no-to-process-porn-and-ceremony` | **no** — a lens, not a type |
| S9 ship | `installer-workmanship`, `release-preparations` | **partly** |

### What "typed" means here, and why only two stages have it

A stage is **typed** when its output is a value another stage can consume without a human reading
it. By that test only **S3** and **S4** qualify: a bead is a row with a schema, and `bv`'s ranking
is a number over a graph.

Everything else hands prose to the next stage. That is the mechanism behind every measured defect
in this plan:

- **S8 has no grade type**, so grading produces four-page documents. §03 measured the cause:
  **6 Verdict-shaped types with no shared trait**, so a grade cannot be a value, only an essay.
- **S5 has no receipt type**, so `success:[N]` from the transport was read as delivery (`cp-z42vu`)
  and a packet vanished.
- **S7 has no completion type**, and §10 Gap 7 found that gap is **precedent-free across 210 mirror
  work-trees** — 14 supervision variants, none meaning *"the worker finished."*

### The composition nobody wrote

The twelve skills are each good and none of them knows about the next. There is no artifact that says
*idea → viability → plan → beads → graph-select → dispatch → work → reap → grade → ship*, with a
typed handoff at each arrow. This document's §11.1 is the closest thing that exists, and it was
written tonight, in response to R13, after nineteen dispatch waves had already run without it.

**That is the honest answer to the question: the process is real, it is distributed, and its
composition is the missing artifact — not another skill, but the typed spine that lets the twelve
compose.** `omp-types` is the crate that would hold those types and has zero dependents.

**NO-CLAIM.** This maps twelve skills to nine stages from `jsm search` output and their
descriptions. It does **not** establish that the twelve are the right twelve, that no thirteenth
exists, or that any of them would compose cleanly if typed — three of the four `jsm` queries tried
returned useful results and one (`"planning workflow beads dispatch"`) returned **No skills found**,
so the search space is not exhausted. The typed/untyped column is a judgement from each skill's
description and this repo's measurements, not from reading all twelve skills end to end.

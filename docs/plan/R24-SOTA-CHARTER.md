# Round 24 — the SOTA lens: measure our plan against the corpus, not against itself

Josh, 2026-09-01: *"are whatever is listed as My requirements truly the best? what makes this
truly state of the art at a planning level? how are we measuring every single item in the list —
if we have all of these surfaces — what must hold true? how are we measuring performance, etc.
across the surfaces of the ecosystem? lets mine some of the planning docs / bead dag graphs of
some of the franken repos."*

Rounds 15–23 graded the plan **against itself** — is this figure current, is this citation live.
Round 24 grades it **against an external corpus we do not control**: 180 mirrored repos at
`/Volumes/ZestData/dicklesworthstone-mirror`. That is the first lens in nine rounds that can
find a *missing category* rather than a stale number.

## What I measured before writing this charter

All commands run 2026-09-01 against the mirror and this repo.

### The corpus does not plan in prose

| artifact | repos (of ~180) |
|---|---:|
| `PLAN.md` | **0** |
| `CHARTER.md` | **0** |
| `ARCHITECTURE.md` | 5 |
| `ROADMAP.md` | 2 |
| `slo.yaml` | 2 |
| `constellation.lock` | 2 |
| `registries/` | 6 |
| **`.beads/`** | **109** |

`.beads` populated in **150** repos, **166,757 beads**, **149,099 closed (89.4%)**, 12,526 open,
2,406 in progress. Largest: `asupersync` 12,456 · `frankenlibc` 7,424 · `frankenterm` 4,975 ·
`franken_engine` 4,537 · `frankenscipy` 4,328.

**Zero planning documents. One hundred fifty bead DAGs.** The corpus's planning substrate is the
graph. Ours is a 13-section, ~1 MB document on its 23rd grading round.

### The corpus's measurement stack, by adoption

| practice | repos | our plan mentions | **in our tree** |
|---|---:|---:|---:|
| `fuzz/` | **34** | **0** | **0** |
| `conformance/` | **32** | 23 | **0** |
| `benches/` | **28** | 17 | **0** |
| `proptest-regressions/` | 4 | **0** | **0** |
| `slo.yaml` | 2 | 16 | — |

CI workflows: `ci.yml` 90 · `release.yml` 62 · `perf.yml` 6 · `fuzz.yml` 6 · `nightly.yml` 5 ·
`bench.yml` 4 · `coverage.yml` 3.

### The three findings that follow, and they are not drift

**F1 — R1–R13 contains no performance requirement.** Across all 13 sections: `throughput` 1
mention, `p95` 1, `p99` **0**, `latency` 4. Josh's question "how are we measuring performance
across the surfaces" currently has no requirement to answer to. This is a MISSING REQUIREMENT,
not an unmet one.

**F2 — the corpus's most-adopted practice is absent from our plan entirely.** `fuzz/` in 34
repos; `fuzz` and `proptest` appear **zero times** in 13 sections. The adversarial-input floor
that is standard equipment across the corpus is not even a declared intent here.

**F3 — we discuss the harnesses ~80 times and have built none.** `oracle` 28 mentions,
`conformance` 23, `differential` 17, `metamorphic` 12 — and `conformance/ 0`, `fuzz/ 0`,
`benches/ 0` directories in this repo. Prose-complete, evidence-empty. Identical shape to the
five absent receipts in `00-brief.md` §2, which is the same defect at a different altitude.

**And an honest correction to our own doctrine:** `AGENTS.md` presents "L2 — SLO-as-code" as a
layer mined from this corpus. `slo.yaml` exists in **2 of ~180 repos**. L2 is a genuine practice
but it is *rare*, not standard; `fuzz/` and `conformance/` at 32–34 are what standard looks like.
The seven-layer standard overstates L2's adoption and should say so.

## The four lanes

Each lane mines the corpus and returns what the plan must ADD — a requirement, a gate, or a
measurement — with a path and a verbatim quote. A lane returning "we already cover this" must
name where.

- **L1 · the flagship DAG** — `asupersync` (12,456 beads) plus `frankenlibc`, `frankenterm`.
  How does a 12k-bead graph stay navigable: label taxonomy, dependency depth and shape, epic
  handling, `close_reason` vocabulary, `compaction_level` lifecycle. Where does the process
  START — what is bead #1 in a repo's history, and what did it demand?
- **L2 · the measurement stack** — the 32 `conformance/` and 34 `fuzz/` trees. What do they
  assert, how are they wired into CI, what is the pass criterion, and what happens on regression.
  Answer Josh's question directly: *given a set of surfaces, what must hold true, and how is
  performance measured across them?*
- **L3 · prose, where it survives** — the 5 `ARCHITECTURE.md`, 2 `ROADMAP.md`, 6 `registries/`,
  2 `constellation.lock`, 2 `slo.yaml`. When the corpus DOES write prose, what does it cover and
  what does it deliberately omit? A 0-for-180 on `PLAN.md` is a choice; characterise it.
- **L4 · how progress is measured** — 149,099 closures. The `close_reason` taxonomy (132,272
  rows), `acceptance_criteria` present on only **26,238 of 166,757 (16%)**, `estimated_minutes`
  on 9,717, `compaction_level` on 160,509. Our `beads-north-star` requires WHAT/WHY/ACCEPTANCE on
  every bead; the corpus carries acceptance on 16%. **Either our doctrine is stricter than the
  SOTA for a reason we can name, or it is ceremony.** Decide which, with evidence.

## The bar

A lane's output is admissible only if every claim carries the command that produced it and a
path into the mirror. "Jeff does X" without a file is a phantom. Where the corpus contradicts our
doctrine, say so — the L4 acceptance-criteria finding above is the model: report the number that
makes us look wrong.

## NO-CLAIM

Adoption counts are `find` over a mirror snapshot; a practice can live under a name I did not
search for, so every zero here is "absent under these names", not "absent". The mirror is a daily
sync and may lag its upstreams. And a corpus is evidence about what this author does, not proof
of what is universally optimal — Josh's question was whether R1–R13 is *the best*, and one
author's 180 repos is the strongest local evidence available, not a literature review.

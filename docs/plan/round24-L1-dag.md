# RETRACTION FIRST — §1's headline was wrong, and Josh caught it

**Retracted claim:** *"The corpus does not start with a plan document — `PLAN.md` 0 repos,
`CHARTER.md` 0 repos. Zero planning documents. One hundred fifty bead DAGs."*

Josh, 2026-09-01: *"the thing is — jeff puts a lot of time into planning — when he starts his
bead dag he already knows what hes planning and building."* He is right and the measurement
that produced my claim was badly shaped: I searched **four filenames at depth 2** and reported
their absence as the absence of planning.

**What is actually there, measured:**

| | asupersync | ours |
|---|---:|---:|
| `docs/**.md` | **581 files, 6.78 MB** | 19 files, 1.41 MB |
| repo-wide `*.md` | **1,043 files, 9.50 MB** | — |
| `*_contract.md` | **99** | 0 |
| `docs/adr/` | **13 ADRs** (15 repos corpus-wide) | 0 |
| mean doc size | **11.7 KB** | **74 KB** |
| docs over 50 KB | **8 of 581 (1%)** | **5 of 19 (26%)** |

**He has ~5x our planning documentation by bytes and 30x by file count.** The opposite of my
claim.

## Where the planning actually lives: `docs/plans/`

```
128141  proposal_to_integrate_ideas_from_nats_into_asupersync__after_feedback.md
 96524  proposal_to_integrate_ideas_from_nats_into_asupersync.md
 69758  plan_to_build_asupersync_in_wasm_for_use_in_browsers.md
 30870  wasm_api_surface_census.md
 12468  wasm_size_perf_budgets.md
  4868  plan_to_port_quic_http3_to_rust.md
```

Three structural facts, and they are the actual SOTA lesson:

**1. One plan per UNDERTAKING, never one plan for the project.** `plan_to_port_quic_http3_to_rust`,
`plan_to_build_asupersync_in_wasm_for_use_in_browsers`. The filename is a verb phrase naming a
specific thing to do. There is no document called "the plan", which is exactly why my filename
search found nothing.

**2. Convergence produces a NEW NAMED ARTIFACT.** `proposal_to_integrate_ideas_from_nats` at
96 KB, then `..._after_feedback` at 128 KB — a second document, +33% content, with the review
state IN THE FILENAME. He does not grade one file twenty-three times; he writes the proposal,
takes feedback, and writes the converged one beside it. Both survive, so the delta is readable.

**3. Ninety-nine `*_contract.md`, mean 11.7 KB.** `failure_domain_contract.md`,
`crash_only_region_contract.md`, `wasm_abi_compatibility_policy.md`. One contract per concern,
named for the concern. This is where the types and invariants get pinned — and it is why bead #4
can say *"Budget type with product semiring semantics"* on day one without ambiguity. **The DAG
references contracts that already exist.**

## So what the day-one phase arc actually proves

§2-§3 below stand as measurements — bead #1 IS a phase epic, the six phases WERE created in one
sitting, formal methods ARE last. But my reading of them inverts. Naming
*"Outcome type with severity lattice"* and *"Budget type with product semiring semantics"* as
beads #2 and #4, minutes apart, is not evidence that planning was skipped. **It is evidence that
planning was finished.** You cannot write those titles without having already settled the
algebra. The DAG opens at the moment the design stops being uncertain.

**The real contrast is shape, not volume.** He writes many small named contracts plus a few
large per-undertaking plans, and converges by writing a successor. We write 19 documents at a
74 KB mean, one of which is a monolith now on its 23rd grading round — 26% of our docs are over
50 KB against his 1%.

---

# Round 24 · L1 — where the projects started and how they arced

Josh, 2026-09-01: *"i want you looking at the beads graph — the actual beads — where did the
projects start and how did they arc."*

Measured directly against `/Volumes/ZestData/dicklesworthstone-mirror`. Every number below has
the command that produced it in §6. Subject: `asupersync`, the largest DAG in the corpus.

---

## 1. The corpus does not start with a plan document

| artifact | repos (of ~180) |
|---|---:|
| `PLAN.md` | **0** |
| `CHARTER.md` | **0** |
| `ARCHITECTURE.md` | 5 |
| `ROADMAP.md` | 2 |
| **`.beads/` populated** | **150** |

**166,757 beads. 149,099 closed (89.4%).** Zero planning documents. This is the single most
decision-relevant fact of the round: we are on grading round 23 of a ~1 MB planning document
that has no analogue anywhere in 180 repos.

## 2. Where it started: the type algebra of the kernel, inside two minutes

`asupersync`, first beads by `created_at`:

```
2026-01-16T06:12  [epic p0]  [EPIC-PHASE] Phase 0 - Single-Thread Deterministic Kernel
2026-01-16T06:13  [task p0]  Implement Outcome type with severity lattice
2026-01-16T06:13  [task p0]  Implement CancelReason type with severity ordering
2026-01-16T06:14  [task p0]  Implement Budget type with product semiring semantics
2026-01-16T06:14  [task p0]  Implement core identifier types (RegionId, TaskId, ObligationId, Time)
```

Bead one is a phase epic. Beads two through five, created in the following **120 seconds**, are
each *"implement `<type>` with `<algebraic property>`"* — a severity **lattice**, a severity
**ordering**, a product **semiring**. The project's first move is to pin the type algebra of its
kernel. Not a survey, not a census, not a requirements document.

## 3. How it arced: all six phases named on day one, formal methods LAST

The 26 `[EPIC-PHASE]` beads, in creation order:

```
2026-01-16  Phase 0 - Single-Thread Deterministic Kernel
2026-01-16  Phase 1 - Parallel Scheduler and Region Heap
2026-01-16  Phase 2 - I/O Integration
2026-01-16  Phase 3 - Actors and Session Types
2026-01-16  Phase 4 - Distributed Structured Concurrency
2026-01-16  Phase 5 - DPOR and TLA+ Tooling
2026-01-18  [SUB-EPIC-PHASE] io_uring Reactor (Linux Modern Async I/O)
2026-01-18  [SUB-EPIC-PHASE] Windows IOCP Reactor
```

**Every phase was created the same day**, then executed for seven months. Two observations that
bear on our situation:

- **The arc is laid down once, in one sitting, and not re-litigated.** Six phases, six titles,
  all `closed`. There is no Phase 0-SUCCESSOR, no re-adoption, no round 23 of the phase list.
- **Formal verification is Phase 5 — dead last.** DPOR and TLA+ come after the kernel, the
  scheduler, I/O, actors, and distribution all exist. Proof follows working code here.

Platform-specific reactors arrive two days later as `SUB-EPIC-PHASE`, which is how scope that was
not visible on day one gets added: a sub-epic under an existing phase, not a new plan.

*(The three ids sharing the Phase 0 title — `bd-akx`, `asupersync-akx`, `asupersync-24my2` — are
an id-migration chain from the `bd` default prefix, not three separate beads.)*

## 4. The execution arc: a DAG drained, not accumulated

| month | created | closed | closed/created |
|---|---:|---:|---:|
| 2026-01 | 2023 | 1671 | 0.83 |
| 2026-02 | **3311** | **3443** | **1.04** |
| 2026-03 | 1647 | 1759 | 1.07 |
| 2026-04 | 2300 | 2182 | 0.95 |
| 2026-05 | 1652 | 1776 | 1.08 |
| 2026-06 | 753 | 562 | 0.75 |
| 2026-07 | 682 | 325 | 0.48 |
| 2026-08 | 89 | 128 | 1.44 |

Closure tracks creation at **~1.0 for five consecutive months**, and in three of them closes
MORE than it opens. Progress is not measured as "percent of plan complete" — it is measured as
**throughput equilibrium**. The DAG is a working queue held near drain, and the ratio falling to
0.48 in July is visible as debt accumulating without anyone having to write a status report.

## 5. What the labels say we were never requiring

1,314 distinct labels. The top of the distribution is a statement of what the work actually is:

| label | beads | our plan |
|---|---:|---|
| `testing` | **931** | — |
| `audit` | 526 | — |
| `dep-plan` | 425 | — |
| `rev-5` / `rev-5-reviewed` | **419 / 408** | — |
| `e2e` | 397 | — |
| `performance` | **268** | `p99` appears **0** times |
| `security` | 223 | — |
| `testing-fuzzing` + `fuzz` | **360** | `fuzz` appears **0** times |
| `proof` / `proof-lane` | 194 / 194 | — |
| `observability` | 190 | — |
| `correctness` | 181 | — |
| `idea-wizard` | 239 | — |
| `mock-code-finder` | 156 | — |

Four things fall out of this table:

**(a) Testing is the largest single category of work.** Not a phase, not a gate — 931 beads.

**(b) Review rounds are BEADS, not document passes.** `rev-5` 419 paired with
`rev-5-reviewed` 408 — a 97% completion rate on a review round, tracked in the graph, with the
reviewed state as its own label. Our convergence rounds produce JSONL ledgers and a 1 MB
document; the corpus's equivalent produces 419 beads that close.

**(c) Performance and fuzzing are first-class labelled work — and are absent from our plan.**
`performance` 268 beads and 360 fuzz beads here; `p99` and `fuzz` appear **zero times** across
our 13 sections. This is the missing-requirement finding, now with a corpus denominator.

**(d) Skills are labels, so a skill's yield is measurable.** `idea-wizard` 239,
`mock-code-finder` 156. The bead carries the name of the skill that generated it, which makes
"did this skill produce work that closed" a query rather than an opinion. We have no equivalent.

## 6. How completion is certified — and where our doctrine is stricter than the SOTA

| field | coverage |
|---|---|
| `close_reason` | **10,918 / 12,457 = 87%**, mean **190 characters** |
| `acceptance_criteria` | **2,140 / 12,457 = 17%** |

First token of `close_reason`, the de-facto vocabulary:

```
COMPLETED 2084   DONE 1961   IMPLEMENTED 1354   FIXED 816   ADDED 352
ALREADY 241   SHIPPED 175   DUPLICATE 110   SUPERSEDED 95   SOUND 91
REPLACED 91   VERIFIED 74   AUDIT 74   STALE 56   VALIDATED 56
```

**This is the finding that makes us look wrong, so it goes first.** Our `beads-north-star`
requires WHAT / WHY / ACCEPTANCE on every bead. The corpus carries acceptance criteria on
**17%** — and a substantive 190-character close reason on **87%**. Its discipline sits at the
**close**, not at the **open**.

Corpus-wide the split is the same: `acceptance_criteria` on 26,238 of 166,757 beads (16%),
`close_reason` on 132,272 (79%).

Either we can name why front-loaded acceptance earns its cost for us, or the requirement is
ceremony we imported. I do not think it is pure ceremony — a bead with acceptance is dispatchable
to an agent that cannot ask questions, and this fleet's measured failure mode is exactly agents
triaging an under-specified bead and going idle rather than shipping. But that is an argument we
now have to *make*, against a 17% baseline, instead of asserting.

## 7. Commands

```bash
M=/Volumes/ZestData/dicklesworthstone-mirror
# planning-doc inventory
for n in PLAN.md ROADMAP.md CHARTER.md ARCHITECTURE.md slo.yaml; do
  echo "$n: $(find $M -maxdepth 2 -name $n | wc -l)"; done
# every measurement in §2-§6: dedupe by id across .beads/*.jsonl, then sort/count
python3 - <<'PY'
import json,pathlib,collections
m=pathlib.Path('/Volumes/ZestData/dicklesworthstone-mirror')
seen={}
for j in (m/'asupersync'/'.beads').glob('*.jsonl'):
    for l in j.read_text().splitlines():
        if l.strip():
            r=json.loads(l)
            seen.setdefault(r.get('id'), r)
rows=sorted(seen.values(), key=lambda r: r.get('created_at') or '')
PY
```

**Dedupe by `id` is load-bearing.** `.beads/` holds several JSONL files per repo; a naive
line-count reports the same bead three or four times, and the first pass of this analysis showed
each Phase-0 title repeated three times for exactly that reason.

## 8. NO-CLAIM

- Adoption counts are `find` over a **daily mirror snapshot** under names I chose. Every zero
  means "absent under these names", never "absent".
- `asupersync` is one repo. The corpus-wide bead and acceptance figures generalise; the phase arc
  in §2-§3 is a single project's shape and the other 149 DAGs are unread as of this document.
- This is evidence about **what one prolific author does**, which is the strongest local evidence
  available. Josh asked whether R1-R13 is *the best*; a 180-repo corpus cannot answer that, only
  whether our requirements omit things a proven practitioner treats as standard. They do: fuzzing
  and performance.
- Reading a label distribution tells us what work was *labelled*, not what was *valuable*.

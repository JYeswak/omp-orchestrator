# plan_to_write_the_document_corpus__after_feedback

Successor to `plan_to_write_the_document_corpus.md` (`cb3df6d`, 02:39Z). Per `/project-startup`
Step 2, convergence writes a **new named artifact** beside the original — both kept, delta
readable — never round N+1 on one file. This is that artifact.

**28 commits landed between the original and this successor.** Every figure below was re-derived
2026-09-02T03:5xZ; none is carried over.

---

## 1. What the original got RIGHT and is now proven

**The document types.** All 18 documents written against the `DPB-*` bar pass it, and — the
decisive measurement — **not one of the 18 breaches the 25 KB ceiling.** The bar is not
aspirational; it is met on every artifact produced under it.

**The order.** Phase 0 (type algebra) before Phase 1 (kernel contracts) held: `subprocess_contract`
cites `BoundedOutcome`, `dispatch_claim_contract` cites `ack_spine_contract`, and
`oracle_comparison_contract` cites `pane_observation_contract` L2. Nothing had to be written twice.

**The corpus-derived taxonomy.** `*_contract`, `*_inventory`, `*_policy`, `error_codes/` — every
document found a natural home in one of them.

## 2. What the original got WRONG

### 2.1 It treated the 78 documents as the work. The PLAN SECTIONS are the work.

| doc set | count | over 25 KB |
|---|---:|---:|
| `docs/contracts/*.md` (written under the bar) | 18 | **0** |
| `docs/plan/*.md` + `PLAN.md` + `ROUNDS.md` | 15 | **15** |

**Every one of the 15 `DPB-SIZE` violations is a pre-existing plan section.** `PLAN.md` 677 KB,
`12-journey.md` 125 KB, `06-gates.md` 76 KB, `00-brief.md` 75 KB, `02-surface-census.md` 66 KB.

The original manifest scheduled 78 NEW documents and said nothing about the 15 oversized ones
already on disk. That is backwards: the new corpus is healthy and the legacy plan is the entire
defect. **Phase 2 of the loop (`PlanShape`) now owns splitting them, and it is not optional
cleanup — it is the reason 23 grading rounds found drift instead of absence.** A section over
25 KB cannot be verified in one sitting, so verification becomes a round, and a round finds only
what moved.

### 2.2 Phase counts were wrong

Phase 0 + 1 planned 13 documents; **18 exist**, because five Phase 2 items landed early:
`ground_truth_contract`, `oracle_comparison_contract`, `pane_readiness_contract`,
`verification_contract`, `degraded_dispatch_policy`. **Phase 2 is 5 of 11 done**, not "in flight".

The manifest's per-phase counts are therefore a plan, not a ledger. They must be re-derived from
disk at each phase boundary, exactly as the `NUMBERS.toml` discipline requires of every other
figure — which the original manifest failed to apply to itself.

### 2.3 It skipped Step 5, and that caused the operational failure of the session

The original went straight from documents to phases without a **program epic**. Measured: 80 beads,
1 epic, **zero program or phase epics**. Consequence, not cosmetic — with no arc in the graph:

- `bv --robot-triage` reported `unblocks=None` on every recommendation;
- every dispatch was the orchestrator hand-picking from a 78-item document;
- the operator had to say "workers idle" **five times** in one session.

**A DAG without an arc is a list.** Filed as `omp-orchestrator-jplf` at 03:5xZ, with the arc in the
title and its closure condition in the body, after the `frankenfs` pattern. Its acceptance
condition 2 — *every open bead is a descendant of exactly one phase epic* — is unmet and is the
next work.

### 2.4 Corpus figures in the original are retired

| original | corrected | why |
|---|---|---|
| 166,757 beads / 150 repos | **126,018 / 109** | un-deduped; `.beads/` holds several JSONL per repo, and 150 counted files not repos |
| `acceptance_criteria` 16% | 14% | same dedupe |
| 17 dual-surface readers, 13 handroll | **12 per repo**, `tick-dispatch` a real depends-but-never-compares | my predicate was wrong; re-derived by `OracleGate` |

Ratios survived dedupe (`close_reason` 79% either way); absolute counts did not.

## 3. What CHANGES in the plan

1. **A new Phase 2.5 — LEGACY PLAN SPLIT.** The 15 oversized sections, split to the bar, with a
   stub at every original path. **Non-negotiable constraint:** `close-evidence-gate` harvests cited
   paths, so a deleted or renamed path invalidates CLOSED beads' evidence — AGENTS.md records that
   exact failure. Split, stub, verify citations, never delete.
2. **Per-phase counts become derived, not declared.** Re-measure from disk at each boundary.
3. **Step 5's acceptance is promoted to a blocker on Phase 3.** No new phase opens until eight
   phase epics exist and every open bead descends from one. This is what stops hip-shooting.
4. **Phase 5 (measurement) moves ahead of Phase 4.** `conformance/ 0 · fuzz/ 0 · benches/ 0`
   against 32/34/28 repos in the corpus, and `fuzz`/`proptest`/`p99` appear **zero times** in our
   13 sections. It is an ABSENT requirement, and the longer it waits the more artifacts get built
   with no harness to hold them.
5. **The `DPB-*` bar gets a gate.** Step 4 is currently a runner nobody enforces; 18 documents
   passing by hand is a habit, not a floor. Ratchet the over-25 KB count from its own scan so it
   can only fall.

## 4. What did NOT change

The 78-document manifest's types, the phase ORDER for 0→3, the pass bar itself, and the
one-concern-per-document rule. Nothing in five hours of measurement contradicted them.

## 5. NO-CLAIM

This successor converges the manifest against measurement taken **inside one session on one
machine**. It does not establish that the eight phases are the right decomposition — only that the
first two produced 18 documents that meet their bar. Phases 4–7 remain unexecuted and unvalidated,
and their document counts are guesses that §2.2's rule now requires be re-derived rather than
trusted. Splitting a 677 KB file makes it verifiable, not correct.

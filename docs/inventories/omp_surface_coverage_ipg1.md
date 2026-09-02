# OMP surface coverage — wave `ipg.1` (PLAN): plan-mode, modes, goals

Bead: `omp-orchestrator-omp-coverage-mission-ipg.1` — Wave PLAN: plan-mode, modes, goals

## Purpose

Classifies the three OMP **PLAN** surfaces against the eight-clause per-crate contract and records the one row this whole census produced that is a live adoption candidate: `goals`, a typed goal runtime with token budgets that we reimplement as bead prose.

It owns the CLASSIFICATION of these 3 surfaces and nothing else. The machine-readable dispositions live in `docs/plan/OMP-COVERAGE-TABLE.jsonl`; the decision to adopt any category-(c) row is a bead, not a sentence here.

Extracted verbatim from `docs/plan/12-journey.md` Appendix B (lines 820–892 at `50df553`); the only transform is a one-level heading demotion.

---

> **ipg.1**: *each surface gets a row in the coverage table with all 8 columns and a classification —
> (a) not ours, (b) reimplemented by scraping, (c) unused capability.*

**Swept 2026-09-01.** Three type roots, 214 files total, walked to symbol level. The per-crate
contract's eight clauses are assessed against our crates, not OMP's — the question is *which clauses
does our ecosystem satisfy for this surface*, not which clauses OMP's own code satisfies.

| surface | OMP files | OMP symbols | 1 asuper | 2 forbid | 3 cancel | 4 typed | 5 logged | 6 observable | 7 robot | 8 WIRED | classification |
|---|---:|---:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|
| `plan-mode` | 6 | 16 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — thin types (file path + title), our plan system is markdown + beads + CONVERGENCE.jsonl |
| `modes` | 204 | 843 | ✓¹ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓¹ | **(b) REIMPLEMENTED BY SCRAPING** — tick-monitor reads the rendered output these modes produce |
| `goals` | 4 | ~30 | — | — | — | ✗ | — | ✗ | ✗ | ✗ | **(c) UNUSED CAPABILITY** — typed goal runtime with token budgets + prompt rendering; we track goals in bead prose |

¹ The ✓s on `modes` are tick-monitor's clauses, not a modes-adopting crate's: tick-monitor
scrapes the pane text that `modes` renders, satisfying observable/logged/typed/robot-reachable at
the output level. No crate adopts the `modes` types themselves. The surface is covered at the
output plane, not at the type plane — and that distinction is the difference between scraping
(which this classification names) and adoption (which none of these surfaces achieves).

** Positive control: FAILED — zero of three surfaces is FULLY COVERED.** This is the honest result,
not a broken scan: all three are agent-plane features (plan approval UX, terminal interaction modes,
goal runtime) and our orchestration layer consumes their *output* (tick-monitor) or *side effect*
(bead prose) without adopting their *types*. The next wave's surfaces should include at least one
we fully cover (e.g. `subprocess-contract`, `receiver-receipt`, or `dispatch-claim-fence` — crates
that exist and are wired), which would satisfy the positive control.

**Anti-vacuity: PASSED** — 3 surfaces enumerated, 214 files walked to symbol level, 0 is not the
count.

#### Per-surface detail

**`plan-mode` — (a) NOT OURS.** The 16 exported symbols offer `PlanApprovalDetails` (file path +
title), `ResolvedApprovedPlan` (file path + content + title), `PlanModelTransition`, `PlanProtection`,
`PlanHandoff`, and plan-file management. 12-journey's own sweep records the honest downgrade:
*"that is a plan reference and an approval flag … it does not supply the grading or convergence
protocol S3 actually needs — which this repo had to build from scratch as CONVERGENCE.jsonl and
convergence.rs."* Our plan system (beads with ACCEPTANCE + CONVERGENCE.jsonl two-lens protocol) is
strictly more capable than a file-path-and-title pair.

**`modes` — (b) REIMPLEMENTED BY SCRAPING.** The 843 exported symbols are the agent's interaction
machinery: composer, autocomplete, orchestrate-keyword detection (`containsOrchestrate`),
workflow-notice rendering (`WORKFLOW_NOTICE`), ultrathink (`ULTRATHINK_NOTICE`), session observer,
skill commands, markdown prose, terminal UI components. tick-monitor reads the pane text that
these modes render — the output, not the types. The scraping approach works (the two-capture rule,
stable-hash stripping, and the exhaustive `classify` match are measured and passing) but it means
every modes rendering change is a potential tick-monitor defect, which is the coupling cost this
classification names.

**`goals` — (c) UNUSED CAPABILITY.** The 4 files offer a typed goal runtime: `GoalRuntimeHost`,
`GoalTurnSnapshot`, `GoalWallClockSnapshot`, `GoalRuntimeSnapshot`, `GoalPromptKind`
(`"active" | "continuation" | "budget-limit"`), `remainingTokens(goal)`, `goalTokenDelta(current,
baseline)`, `renderGoalPrompt(kind, goal)`, `renderTrustedObjective(objective)`. The two features
our ecosystem lacks and OMP provides: **token budgeting** (per-goal token deltas against a baseline,
which would ground §8.2 Q2's cost question) and **prompt-kind-aware rendering** (active /
continuation / budget-limit prompts, which would make the dispatch packet builder type-safe). We
track goals in bead prose; OMP tracks them with typed runtime snapshots and wall-clock budgets.
The gap is real and the surface is adoptable — but adoption is a decision for the S5 Cost field,
not this mapping.

#### What would Jeffrey do

`goals` is the one surface where the mirror has prior art: `asupersync`'s obligation-ledger pattern
(`src/obligation/crdt.rs`, `CrdtObligationLedger`) types the same shape — a long-running objective
with budget constraints and periodic checkpoints. We already depend on asupersync; the obligation
types are one `use` away. The gap is not the vocabulary (OMP's `goals` and asupersync's `obligation`
are the same concept) but the adoption decision: neither surface is consumed, and building a
third goal-tracker beside beads and the OMP goal runtime would be the 20-mechanisms defect.

NO-CLAIM: mapping is not adopting. (a) not-ours is a legitimate terminal state. The coverage table
records what exists; the build decision is §09's, not Appendix B's.

## Cross-References

- `docs/inventories/omp_surface_coverage_index.md` — the eleven-wave roll-up, and the wave that has no document
- `docs/plan/12-journey.md` — the nine-stage runbook this appendix was extracted from
- `docs/plan/OMP-COVERAGE-TABLE.jsonl` — machine-readable dispositions
- `.beads/issues.jsonl` — `omp-orchestrator-omp-coverage-mission-ipg.1`

## Validation

Derives both sides and compares them: the surface set this document tabulates, against the
surface set its bead declares. It quotes neither, so it cannot go stale the way a copied list
does. An empty parse on either side is FAIL, never PASS.

```bash
cd /Users/josh/Developer/omp-orchestrator && W=1 && \
D=$(sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' "docs/inventories/omp_surface_coverage_ipg${W}.md" \
    | grep -vE '^(surface|-+)$' | sort -u) && \
B=$(jq -r --arg id "omp-orchestrator-omp-coverage-mission-ipg.${W}" \
      'select(.id==$id)|.title' .beads/issues.jsonl | head -1 \
    | sed 's/^[^:]*: *//' | tr ',' '\n' | tr -d ' ' | sed '/^$/d' | sort -u) && \
[ -n "$D" ] && [ -n "$B" ] && diff <(printf '%s\n' "$D") <(printf '%s\n' "$B") \
  && echo "PASS ipg.${W}" || echo "FAIL ipg.${W}"
```

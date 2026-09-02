# ROUNDS — every grading round, in one document

> **GENERATED.** `cargo run -p convergence-stamp -- --write`. Do not hand-edit; re-run it.
>
> **Josh, 2026-09-01:** *"make sure we get all rounds of edits into a single doc — no more rounds of convergence until we have a stamped doc with all rounds included in it. only then can we start another round."*


Before this document existed the round record was split across twelve files with two halves that no reader could reconcile: `CONVERGENCE.jsonl` held rounds 8-15 and 22, while rounds **16 through 21 existed only in eleven per-agent `round*.jsonl` files** and had never reached the canonical ledger. Round 11 is absent from both. `CONVERGENCE.md`, the human-readable convergence document, is hand-written prose that stops at round 10. So "have we converged?" had no answer a single artifact could give, and a new round could start while six prior rounds were unrepresented.

This document is the answer, and `STAMP.toml` is its coverage claim. A new round is refused while any round on disk is missing from the stamp, or any plan section has changed since the stamp was cut.

## Stamp

| field | value |
|---|---|
| cut at | `epoch:1788307780` |
| rounds covered | **14** |
| round rows | **174** |
| declared findings | **559** |
| dispositioned in FINDINGS.jsonl | **220** |
| plan sections digested | **13** |

**220 of 559 declared findings are dispositioned.** That ratio is the honest state of the plan, and a current stamp does not improve it — the stamp says the RECORD is complete, never that the work is.

## Every round

| round | rows | declared | dispositioned | sections graded | source files |
|---:|---:|---:|---:|---:|---|
| **8** | 12 | 9 | 0 | 12 | `docs/plan/CONVERGENCE.jsonl` |
| **9** | 12 | 13 | 0 | 12 | `docs/plan/CONVERGENCE.jsonl` |
| **10** | 12 | 77 | 0 | 12 | `docs/plan/CONVERGENCE.jsonl` |
| **12** | 9 | 89 | 0 | 9 | `docs/plan/CONVERGENCE.jsonl` |
| **13** | 13 | 80 | 0 | 13 | `docs/plan/CONVERGENCE.jsonl` |
| **14** | 12 | 72 | 0 | 12 | `docs/plan/CONVERGENCE.jsonl` |
| **15** | 13 | 20 | 21 | 13 | `docs/plan/CONVERGENCE.jsonl` |
| **16** | 20 | 76 | 89 | 14 | `docs/plan/round16-AdversaryEye.jsonl`<br>`docs/plan/round16-DeltaEye.jsonl`<br>`docs/plan/round16-GreenFrog.jsonl`<br>`docs/plan/round16-SchemaEye.jsonl`<br>`docs/plan/round16-TraceEye.jsonl` |
| **17** | 4 | 0 | 0 | 4 | `docs/plan/round17-GreenFrog.jsonl` |
| **18** | 4 | 0 | 0 | 4 | `docs/plan/round18-GreenFrog.jsonl` |
| **19** | 15 | 47 | 47 | 15 | `docs/plan/round19-GreenFrog.jsonl` |
| **20** | 14 | 25 | 25 | 14 | `docs/plan/round20-GreenFrog.jsonl` |
| **21** | 14 | 38 | 38 | 14 | `docs/plan/round21-GreenFrog.jsonl` |
| **22** | 20 | 13 | 0 | 13 | `docs/plan/CONVERGENCE.jsonl`<br>`docs/plan/round16-Opus.jsonl` |

### Unreconciled: 7 rounds, 353 declared findings, zero dispositioned

| round | declared | dispositioned |
|---:|---:|---:|
| 8 | 9 | 0 |
| 9 | 13 | 0 |
| 10 | 77 | 0 |
| 12 | 89 | 0 |
| 13 | 80 | 0 |
| 14 | 72 | 0 |
| 22 | 13 | 0 |

HD-0006 scopes reconciliation to rounds 15-22. Any round above that is outside both the ruling and the ledger — it needs a ruling or a bead, not a silent pass.

## Every row

| round | section | lens | graded by | declared | verdict | source |
|---:|---|---|---|---:|---|---|
| 8 | `00-brief` | investor | <none> | 1 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 8 | `01-idea` | investor | <none> | 2 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 8 | `02-surface-census` | investor | <none> | 3 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 8 | `03-crates` | adversarial | <none> | 0 | PASS | `docs/plan/CONVERGENCE.jsonl` |
| 8 | `04-diagrams` | adversarial | <none> | 1 | PASS-WITH-FIXES | `docs/plan/CONVERGENCE.jsonl` |
| 8 | `05-actions` | adversarial | <none> | 0 | PASS | `docs/plan/CONVERGENCE.jsonl` |
| 8 | `06-gates` | absence | <none> | 0 | PASS | `docs/plan/CONVERGENCE.jsonl` |
| 8 | `07-installability` | absence | <none> | 0 | PASS | `docs/plan/CONVERGENCE.jsonl` |
| 8 | `08-end-users` | absence | <none> | 0 | PASS | `docs/plan/CONVERGENCE.jsonl` |
| 8 | `09-milestones` | evidence | <none> | 1 | PASS-WITH-FIXES | `docs/plan/CONVERGENCE.jsonl` |
| 8 | `10-prior-art` | evidence | <none> | 0 | PASS | `docs/plan/CONVERGENCE.jsonl` |
| 8 | `11-lifecycle` | evidence | <none> | 1 | PASS-WITH-FIXES | `docs/plan/CONVERGENCE.jsonl` |
| 9 | `00-brief` | evidence | <none> | 4 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 9 | `01-idea` | evidence | <none> | 3 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 9 | `02-surface-census` | evidence | <none> | 2 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 9 | `03-crates` | investor | <none> | 0 | PASS | `docs/plan/CONVERGENCE.jsonl` |
| 9 | `04-diagrams` | investor | <none> | 0 | PASS | `docs/plan/CONVERGENCE.jsonl` |
| 9 | `05-actions` | investor | <none> | 0 | PASS | `docs/plan/CONVERGENCE.jsonl` |
| 9 | `06-gates` | adversarial | <none> | 0 | PASS | `docs/plan/CONVERGENCE.jsonl` |
| 9 | `07-installability` | adversarial | <none> | 1 | PASS-WITH-FIXES | `docs/plan/CONVERGENCE.jsonl` |
| 9 | `08-end-users` | adversarial | <none> | 1 | PASS-WITH-FIXES | `docs/plan/CONVERGENCE.jsonl` |
| 9 | `09-milestones` | absence | <none> | 0 | PASS | `docs/plan/CONVERGENCE.jsonl` |
| 9 | `10-prior-art` | absence | <none> | 1 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 9 | `11-lifecycle` | absence | <none> | 1 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 10 | `00-brief` | absence | <none> | 5 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 10 | `01-idea` | absence | <none> | 7 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 10 | `02-surface-census` | absence | <none> | 5 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 10 | `03-crates` | evidence | <none> | 1 | UN-CONVERGED | `docs/plan/CONVERGENCE.jsonl` |
| 10 | `04-diagrams` | evidence | <none> | 5 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 10 | `05-actions` | evidence | <none> | 1 | UN-CONVERGED | `docs/plan/CONVERGENCE.jsonl` |
| 10 | `06-gates` | investor | <none> | 3 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 10 | `07-installability` | investor | <none> | 7 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 10 | `08-end-users` | investor | <none> | 4 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 10 | `09-milestones` | adversarial | <none> | 16 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 10 | `10-prior-art` | adversarial | <none> | 13 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 10 | `11-lifecycle` | adversarial | <none> | 10 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 12 | `00-brief` | adversarial | <none> | 16 | PASS-WITH-FIXES | `docs/plan/CONVERGENCE.jsonl` |
| 12 | `01-idea` | adversarial | <none> | 19 | PASS-WITH-FIXES | `docs/plan/CONVERGENCE.jsonl` |
| 12 | `02-surface-census` | adversarial | <none> | 12 | PASS-WITH-FIXES | `docs/plan/CONVERGENCE.jsonl` |
| 12 | `03-crates` | absence | <none> | 10 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 12 | `04-diagrams` | absence | <none> | 6 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 12 | `05-actions` | absence | <none> | 6 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 12 | `06-gates` | evidence | <none> | 6 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 12 | `07-installability` | evidence | <none> | 8 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 12 | `08-end-users` | evidence | <none> | 6 | FAIL | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `00-brief` | fresh-eyes-severity | GradeBrief | 7 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `01-idea` | fresh-eyes-severity | GradeIdea | 7 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `02-surface-census` | fresh-eyes-severity | GradeCensus | 5 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `03-crates` | fresh-eyes-severity | GradeCrates | 4 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `04-diagrams` | fresh-eyes-severity | GradeDiagrams | 6 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `05-actions` | fresh-eyes-severity | GradeActions | 7 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `06-gates` | fresh-eyes-severity | GradeGates | 10 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `07-installability` | fresh-eyes-severity | GradeInstall | 8 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `08-end-users` | fresh-eyes-severity | GradeEndUsers | 4 | MAJOR_OPEN | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `09-milestones` | fresh-eyes-severity | orchestrator-pane-1 (claude-opus-5) | 5 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `10-prior-art` | fresh-eyes-severity | orchestrator-pane-1 (claude-opus-5) | 3 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `11-lifecycle` | fresh-eyes-severity | orchestrator-pane-1 (claude-opus-5) | 8 | MAJOR_OPEN | `docs/plan/CONVERGENCE.jsonl` |
| 13 | `12-journey` | fresh-eyes-severity | GradeJourney | 6 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 14 | `00-brief` | operator-at-3am | held-out scout lens, zero shared context | 6 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 14 | `01-idea` | operator-at-3am | held-out scout lens, zero shared context | 3 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 14 | `02-surface-census` | operator-at-3am | held-out scout lens, zero shared context | 9 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 14 | `03-crates` | operator-at-3am | held-out scout lens, zero shared context | 8 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 14 | `04-diagrams` | operator-at-3am | held-out scout lens, zero shared context | 4 | MAJOR_OPEN | `docs/plan/CONVERGENCE.jsonl` |
| 14 | `05-actions` | operator-at-3am | held-out scout lens, zero shared context | 8 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 14 | `06-gates` | operator-at-3am | held-out scout lens, zero shared context | 7 | MAJOR_OPEN | `docs/plan/CONVERGENCE.jsonl` |
| 14 | `07-installability` | operator-at-3am | held-out scout lens, zero shared context | 8 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 14 | `08-end-users` | operator-at-3am | held-out scout lens, zero shared context | 7 | MAJOR_OPEN | `docs/plan/CONVERGENCE.jsonl` |
| 14 | `09-milestones` | operator-at-3am | held-out scout lens, zero shared context | 7 | MAJOR_OPEN | `docs/plan/CONVERGENCE.jsonl` |
| 14 | `10-prior-art` | operator-at-3am | held-out scout lens, zero shared context | 5 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 14 | `11-lifecycle` | operator-at-3am | held-out scout lens, zero shared context | 0 | ACTIONABLE | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `00-brief` | rule-zero | GreenFrog | 1 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `01-idea` | rule-zero | GreenFrog | 1 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `02-surface-census` | rule-zero | GreenFrog | 1 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `03-crates` | rule-zero | GreenFrog | 1 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `04-diagrams` | rule-zero | BlueLantern | 2 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `05-actions` | rule-zero | BlueLantern | 3 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `06-gates` | rule-zero | BlueLantern | 3 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `07-installability` | rule-zero | AmberGate | 1 | ACTIONABLE | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `08-end-users` | rule-zero | AmberGate | 2 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `09-milestones` | rule-zero | AmberGate | 1 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `10-prior-art` | rule-zero | SilverWolf | 2 | ACTIONABLE | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `11-lifecycle` | rule-zero | SilverWolf | 1 | BLOCKED | `docs/plan/CONVERGENCE.jsonl` |
| 15 | `12-journey` | rule-zero | SilverWolf | 1 | MAJOR_OPEN | `docs/plan/CONVERGENCE.jsonl` |
| 16 | `00-brief` | adversarial | AdversaryEye | 4 | MAJOR_OPEN | `docs/plan/round16-AdversaryEye.jsonl` |
| 16 | `00-brief` | fresh-eyes-rule-zero | GreenFrog | 1 | BLOCKED | `docs/plan/round16-GreenFrog.jsonl` |
| 16 | `01-idea` | adversarial | AdversaryEye | 3 | MAJOR_OPEN | `docs/plan/round16-AdversaryEye.jsonl` |
| 16 | `01-idea` | fresh-eyes-rule-zero | GreenFrog | 1 | BLOCKED | `docs/plan/round16-GreenFrog.jsonl` |
| 16 | `02-surface-census` | DELTA | DeltaEye | 5 | BLOCKED | `docs/plan/round16-DeltaEye.jsonl` |
| 16 | `02-surface-census` | fresh-eyes-rule-zero | GreenFrog | 1 | BLOCKED | `docs/plan/round16-GreenFrog.jsonl` |
| 16 | `03-crates` | DELTA | DeltaEye | 10 | BLOCKED | `docs/plan/round16-DeltaEye.jsonl` |
| 16 | `03-crates` | fresh-eyes-rule-zero | GreenFrog | 1 | BLOCKED | `docs/plan/round16-GreenFrog.jsonl` |
| 16 | `04-diagrams` | BUYER-TRACE | TraceEye | 4 | MAJOR_OPEN | `docs/plan/round16-TraceEye.jsonl` |
| 16 | `05-actions` | BUYER-TRACE | TraceEye | 6 | MAJOR_OPEN | `docs/plan/round16-TraceEye.jsonl` |
| 16 | `06-gates` | DELTA | DeltaEye | 5 | MAJOR_OPEN | `docs/plan/round16-DeltaEye.jsonl` |
| 16 | `06-gates` | schema-consistency | SchemaEye | 4 | MAJOR_OPEN | `docs/plan/round16-SchemaEye.jsonl` |
| 16 | `07-installability` | adversarial | AdversaryEye | 4 | MAJOR_OPEN | `docs/plan/round16-AdversaryEye.jsonl` |
| 16 | `08-end-users` | BUYER-TRACE | TraceEye | 2 | ACTIONABLE | `docs/plan/round16-TraceEye.jsonl` |
| 16 | `09-milestones` | DELTA | DeltaEye | 4 | MAJOR_OPEN | `docs/plan/round16-DeltaEye.jsonl` |
| 16 | `09-milestones` | schema-consistency | SchemaEye | 3 | MAJOR_OPEN | `docs/plan/round16-SchemaEye.jsonl` |
| 16 | `10-prior-art` | BUYER-TRACE | TraceEye | 1 | ACTIONABLE | `docs/plan/round16-TraceEye.jsonl` |
| 16 | `11-lifecycle` | schema-consistency | SchemaEye | 6 | BLOCKED | `docs/plan/round16-SchemaEye.jsonl` |
| 16 | `12-journey` | adversarial | AdversaryEye | 5 | MAJOR_OPEN | `docs/plan/round16-AdversaryEye.jsonl` |
| 16 | `CONVERGENCE` | schema-consistency | SchemaEye | 6 | BLOCKED | `docs/plan/round16-SchemaEye.jsonl` |
| 17 | `00-brief` | fresh-eyes-rule-zero | GreenFrog | 0 | BLOCKED | `docs/plan/round17-GreenFrog.jsonl` |
| 17 | `01-idea` | fresh-eyes-rule-zero | GreenFrog | 0 | BLOCKED | `docs/plan/round17-GreenFrog.jsonl` |
| 17 | `02-surface-census` | fresh-eyes-rule-zero | GreenFrog | 0 | BLOCKED | `docs/plan/round17-GreenFrog.jsonl` |
| 17 | `03-crates` | fresh-eyes-rule-zero | GreenFrog | 0 | BLOCKED | `docs/plan/round17-GreenFrog.jsonl` |
| 18 | `00-brief` | fresh-eyes-rule-zero | GreenFrog | 0 | BLOCKED | `docs/plan/round18-GreenFrog.jsonl` |
| 18 | `01-idea` | fresh-eyes-rule-zero | GreenFrog | 0 | BLOCKED | `docs/plan/round18-GreenFrog.jsonl` |
| 18 | `02-surface-census` | fresh-eyes-rule-zero | GreenFrog | 0 | BLOCKED | `docs/plan/round18-GreenFrog.jsonl` |
| 18 | `03-crates` | fresh-eyes-rule-zero | GreenFrog | 0 | BLOCKED | `docs/plan/round18-GreenFrog.jsonl` |
| 19 | `00-brief` | fresh-independent-plan-reader | round19-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `01-idea` | fresh-independent-plan-reader | round19-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `02-surface-census` | fresh-independent-plan-reader | round19-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `03-crates` | fresh-independent-plan-reader | round19-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `04-diagrams` | fresh-independent-plan-reader | round19-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `05-actions` | fresh-independent-plan-reader | round19-fresh-eyes | 0 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `06-gates` | fresh-independent-plan-reader | round19-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `07-installability` | fresh-independent-plan-reader | round19-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `08-end-users` | fresh-independent-plan-reader | round19-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `09-milestones` | fresh-independent-plan-reader | round19-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `10-prior-art` | fresh-independent-plan-reader | round19-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `11-lifecycle` | fresh-independent-plan-reader | round19-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `12-journey` | fresh-independent-plan-reader | round19-fresh-eyes | 6 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `cross-section` | fresh-independent-plan-reader | round19-fresh-eyes | 15 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 19 | `cross-section-more` | fresh-independent-plan-reader | round19-fresh-eyes | 12 | UN-CONVERGED | `docs/plan/round19-GreenFrog.jsonl` |
| 20 | `00-brief` | executable-contract-provenance-cross-section | round20-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `01-idea` | executable-contract-provenance-cross-section | round20-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `02-surface-census` | executable-contract-provenance-cross-section | round20-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `03-crates` | executable-contract-provenance-cross-section | round20-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `04-diagrams` | executable-contract-provenance-cross-section | round20-fresh-eyes | 0 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `05-actions` | executable-contract-provenance-cross-section | round20-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `06-gates` | executable-contract-provenance-cross-section | round20-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `07-installability` | executable-contract-provenance-cross-section | round20-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `08-end-users` | executable-contract-provenance-cross-section | round20-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `09-milestones` | executable-contract-provenance-cross-section | round20-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `10-prior-art` | executable-contract-provenance-cross-section | round20-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `11-lifecycle` | executable-contract-provenance-cross-section | round20-fresh-eyes | 3 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `12-journey` | executable-contract-provenance-cross-section | round20-fresh-eyes | 3 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 20 | `cross-section` | executable-contract-provenance-cross-section | round20-fresh-eyes | 3 | UN-CONVERGED | `docs/plan/round20-GreenFrog.jsonl` |
| 21 | `00-brief` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `01-idea` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 0 | ZERO_NEW | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `02-surface-census` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `03-crates` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `04-diagrams` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `05-actions` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 4 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `06-gates` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 2 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `07-installability` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 5 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `08-end-users` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 5 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `09-milestones` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 3 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `10-prior-art` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `11-lifecycle` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 1 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `12-journey` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 7 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 21 | `cross-cutting` | adversarial-acceptance-structural-honesty | round21-fresh-eyes | 3 | UN-CONVERGED | `docs/plan/round21-GreenFrog.jsonl` |
| 22 | `00-brief` | extraction-truth-replayability | GreenFrog | 0 | ZERO_NEW | `docs/plan/CONVERGENCE.jsonl` |
| 22 | `00-brief` | reproducibility | Opus | 1 | MAJOR_OPEN | `docs/plan/round16-Opus.jsonl` |
| 22 | `00-brief` | reproducibility | Opus | 0 | ACTIONABLE | `docs/plan/round16-Opus.jsonl` |
| 22 | `01-idea` | extraction-truth-replayability | GreenFrog | 0 | ZERO_NEW | `docs/plan/CONVERGENCE.jsonl` |
| 22 | `01-idea` | reproducibility | Opus | 1 | BLOCKED | `docs/plan/round16-Opus.jsonl` |
| 22 | `01-idea` | reproducibility | Opus | 0 | ACTIONABLE | `docs/plan/round16-Opus.jsonl` |
| 22 | `02-surface-census` | extraction-truth-replayability | GreenFrog | 0 | ZERO_NEW | `docs/plan/CONVERGENCE.jsonl` |
| 22 | `02-surface-census` | reproducibility | Opus | 1 | ACTIONABLE | `docs/plan/round16-Opus.jsonl` |
| 22 | `03-crates` | extraction-truth-replayability | GreenFrog | 0 | ZERO_NEW | `docs/plan/CONVERGENCE.jsonl` |
| 22 | `03-crates` | reproducibility | Opus | 2 | BLOCKED | `docs/plan/round16-Opus.jsonl` |
| 22 | `03-crates` | reproducibility | Opus | 0 | BLOCKED | `docs/plan/round16-Opus.jsonl` |
| 22 | `04-diagrams` | reproducibility | Opus | 0 | ACTIONABLE | `docs/plan/round16-Opus.jsonl` |
| 22 | `05-actions` | reproducibility | Opus | 0 | ACTIONABLE | `docs/plan/round16-Opus.jsonl` |
| 22 | `06-gates` | reproducibility | Opus | 3 | BLOCKED | `docs/plan/round16-Opus.jsonl` |
| 22 | `07-installability` | reproducibility | Opus | 3 | MAJOR_OPEN | `docs/plan/round16-Opus.jsonl` |
| 22 | `08-end-users` | reproducibility | Opus | 1 | MAJOR_OPEN | `docs/plan/round16-Opus.jsonl` |
| 22 | `09-milestones` | reproducibility | Opus | 0 | ACTIONABLE | `docs/plan/round16-Opus.jsonl` |
| 22 | `10-prior-art` | reproducibility | Opus | 0 | ACTIONABLE | `docs/plan/round16-Opus.jsonl` |
| 22 | `11-lifecycle` | reproducibility | Opus | 0 | ACTIONABLE | `docs/plan/round16-Opus.jsonl` |
| 22 | `12-journey` | reproducibility | Opus | 1 | MAJOR_OPEN | `docs/plan/round16-Opus.jsonl` |

## Section digests at this stamp

| section | sha256 |
|---|---|
| `00-brief.md` | `dec2ba3ca0542238` |
| `01-idea.md` | `d9ebda567994fa57` |
| `02-surface-census.md` | `a4f438ffc5474525` |
| `03-crates.md` | `f7beefaf8249c5d1` |
| `04-diagrams.md` | `152a14cb030c0740` |
| `05-actions.md` | `82ca124497b46954` |
| `06-gates.md` | `ba344de286b3734b` |
| `07-installability.md` | `8f6fd3397d1cc2eb` |
| `08-end-users.md` | `762f8031ab14db9b` |
| `09-milestones.md` | `6d6c09d52380cc48` |
| `10-prior-art.md` | `fc4dd89abb9bdeb3` |
| `11-lifecycle.md` | `3e5499ddf00d6d00` |
| `12-journey.md` | `bf5014a6809c6384` |

## NO-CLAIM

A current stamp means the record is COMPLETE and the sections have not moved since it was cut. It does not mean the plan is converged, correct, or good, and it does not mean the declared findings were addressed — the dispositioned column is the only thing that speaks to that. A stamp over a plan with hundreds of undispositioned findings is a valid stamp of an unfinished plan.

This document is GENERATED from `docs/plan/CONVERGENCE.jsonl`, every `docs/plan/round*.jsonl`, `docs/plan/FINDINGS.jsonl`, and the numbered section files. It discovers its inputs from disk rather than a hand-listed set, because a hand-listed set is exactly how rounds 16-21 went unnoticed.

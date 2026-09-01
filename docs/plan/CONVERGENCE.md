# CONVERGENCE — the finish condition for this plan

> **Josh, 2026-08-31:** *"we need to ensure every section of the plan has 2 rounds of no new
> findings — once all sections are done"* → then, and only then, the plan becomes a `br`/`bv` DAG.

## The rule

A section is **CONVERGED** when it has **two consecutive graded rounds with zero new findings,
under two different lenses.**

Two lenses, not one, because a single lens returning clean twice proves the lens stopped looking,
not that the section is sound. Every lens in this session has had a blind spot another lens caught:

| lens | what it caught that others missed |
|---|---|
| investor | the document never states what anything costs |
| adversarial | a table with five rows called "four-layer" |
| absence | no economic dimension exists anywhere in eleven sections |
| evidence | the self-correction count contradicting itself |

## The ledger

`CONVERGENCE.jsonl`, one row per graded (section, round, lens):

```json
{"section":"06-gates","round":8,"lens":"adversarial","new_findings":0,"verdict":"PASS","evidence":"/tmp/grade/r8-06.md"}
```

**`new_findings` is declared by the grader, not inferred.** A first attempt at this ledger derived
it by pattern-matching grade files and reported `06-gates` at a 2-round clean streak while `%1409`
had just failed it. Inherited evidence is not evidence — the same defect `%1408` found in 63 retire
rows. **The clock therefore starts at zero:** rounds 1–7 were not run under this contract and do
not count toward it, however much real work they did.

## Zero is a claim, not a shrug

`new_findings: 0` means *"I looked with this lens and found nothing new."* It is the strongest
claim a grader makes, and it is the one to distrust. A grader returning 0 must name **what it
searched and how** — a zero that cannot describe its search space is a silence, and this session
has produced eleven false zeros that looked exactly like measurements.

## What converged does not mean

Converged means **no lens found anything new, twice running**. It does not mean correct, complete,
or that the thing described will work. It means the reviewing process stopped producing signal —
which is a fact about the process, and only weak evidence about the document.

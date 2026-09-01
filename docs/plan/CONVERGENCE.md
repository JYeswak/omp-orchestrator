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

---

## The held-out lens — reserved, unused, and not to be spent early

Adopted from `generic_aar/README.md`: *"one held-out benchmark (a different distribution / a fresh
set, to test that a fix generalizes rather than overfits)."*

**The held-out lens is `operator-at-3am`, and it has not been used in any round.**

> *You are woken at 3am. A stage is next, a pane is free, and you have no context. Using only this
> section, what exactly do you send, and how will you know it worked? Every place you would have to
> ask someone is a finding.*

It is reserved because the four working lenses have now read each other's findings in the ledger
across three rounds. That is the overfitting the held-out leg exists to detect: a section can go
clean because the graders converged on one another rather than because it is sound.

### Rules

1. **It runs once**, across all twelve sections, after the hill-climbing rounds report converged.
2. **No pane sees this prompt before then.** A lens that has been anticipated is not held out —
   which is exactly why the wording above is the whole of it and there is no rubric to prepare for.
3. **A section that converged under two lenses and then fails this one did not converge.** It was
   ground smooth against the graders it had met, and its two clean rounds are evidence about the
   graders rather than about the section.
4. `convergence.rs` **refuses the DAG conversion** until a `held_out` row exists for every section.

### Why this lens and not another

The four in rotation ask *is it true* (evidence), *is it contradictory* (adversarial), *what is
absent* (absence), *is it worth money* (investor). None asks **can it be executed by someone who was
not here** — and §12's whole claim is that a stage which cannot be dispatched is a paragraph. The
held-out lens tests the property the plan is *for*, and it is the one property twelve sections of
self-grading cannot establish about themselves.

**NO-CLAIM.** One lens is a thin held-out set. AAR holds out a whole benchmark on a different
distribution; we hold out one reader. It detects lens-adaptation, which is what we can afford to
test, and it does not test generalization to a different *project* — which is the thing S1 inception
actually needs and which nothing here checks.

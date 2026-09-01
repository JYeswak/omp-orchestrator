# CONVERGENCE — the finish condition for this plan

> **Josh, 2026-08-31:** *"we need to ensure every section of the plan has 2 rounds of no new
> findings — once all sections are done"* → then, and only then, the plan becomes a `br`/`bv` DAG.

## MEASURED 2026-08-31: rounds 8 and 9 measured the graders, not the plan

Round 10 ran with **fresh eyes** — panes dispatched subagents that had never read the ledger, rather
than grading from three rounds of accumulated context. One variable changed. The result:

| round | protocol | findings | converged |
|---|---|---:|---:|
| 8 | four lenses, panes grade directly | ~14 | 6 sections clean |
| 9 | same lenses rotated, panes grade directly | 9 | **3 CONVERGED** |
| 10 | same lenses, **fresh subagents, zero shared context** | **77** | **0** |

**All three banked sections fell.** Not one, not two — three of three:

```
03-crates    r8:0 (adversarial) -> r9:0 (investor)    -> r10:1  UN-CONVERGED
05-actions   r8:0 (adversarial) -> r9:0 (investor)    -> r10:1  UN-CONVERGED
06-gates     r8:0 (absence)     -> r9:0 (adversarial) -> r10:3  REGRESSED
```

`09-milestones` went from 1 finding to **16**. `10-prior-art` to 13. `11-lifecycle` to 10.

**Two consecutive zeros under two different lenses turned out to be worth nothing**, because both
lenses were carried by agents who had read every prior finding in this ledger. The rule was written
to prevent one lens going blind and it did not survive contact with the actual failure mode: *all
four lenses going blind together, in the same direction, for the same reason.*

### What this costs and what it buys

Rounds 8 and 9 are not deleted — the fixes they produced were real, and several were load-bearing.
What is retracted is the **convergence claim**. Those rounds measured a property of the graders.

The clock is at **0/12** and the standard is now fresh eyes. It is 3–5× slower per round: `%1413`
finished in ~10 minutes; the others took 26–50. That is the price of a reader who was not here.

### The un-conversions were honest, and that matters more than the count

`%1409` held `03-crates` and `05-actions` — sections **it had itself graded clean in round 9** — and
un-converged both rather than protect the number. `%1413` did the same to `06-gates`. Every one was
re-derived by the pane before recording. A floor that only fires on someone else's work is not a
floor.

## The rule

A section is **CONVERGED** when it has **two consecutive graded rounds with zero new findings,
under two different lenses, both graded by readers with no prior exposure to this ledger.**

The third clause was added after round 10 refuted the first two. Two lenses is necessary and was
never sufficient: a lens is a *question*, and four different questions asked by four agents who have
all read the same findings converge on the same blind spot. **Fresh eyes is not an optimisation of
this protocol — it is the load-bearing term.**

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

---

## The rate curve, measured across 12 rounds — and why zero was the wrong target

Josh's terminating condition: *"2 rounds of 0 new findings with fresh eyes prompts."*
Measured 2026-09-01, all rounds in `CONVERGENCE.jsonl`:

| round | findings/section | what changed in the protocol |
|---|---:|---|
| 8 | 0.8 | graders reading their own accumulated context |
| 9 | 1.1 | same |
| 10 | **6.4** | fresh-eyes subagents, zero shared context |
| 12 | **9.9** | severity required + fresh eyes |
| 13 | **5.3** | severity + fresh eyes, first round tagged |

**The rate climbed with every protocol improvement.** That is not a document getting
worse — it is each improvement removing a blind spot. Round 10 established the
mechanism in its own finding: *"two consecutive zeros on 06-gates were evidence
about the graders, not the section."* Rounds 8–9 measured 0.8–1.1 because the
graders were re-reading their own prior work; fresh eyes raised it 6.4×.

So the condition as literally written has a perverse property: **the fastest way to
reach two zero rounds is to make the graders worse.** Rounds 8–9 nearly satisfied
it, and both were retracted as non-banking once `gates_green` was backfilled
truthfully. A stop condition that a weaker grader satisfies sooner is measuring the
grader, not the artifact.

### What round 13 makes possible

Round 13 is the first round whose findings carry a class:

| section | BLOCKER | MAJOR | MINOR |
|---|---:|---:|---:|
| 09-milestones | 1 | 3 | 1 |
| 10-prior-art | 1 | 2 | 0 |
| 11-lifecycle | 0 | 3 | 5 |
| **total (3 of 12)** | **2** | **8** | **6** |

Two BLOCKERs. That is a **countable, closable** condition in a way "89 findings" was
not — and the rate fell 9.9 → 5.3 in the same round severity became mandatory,
consistent with graders having to justify each finding's class rather than list
everything they noticed.

### The amendment — ADOPTED by Josh 2026-09-01 (HD-0005), superseding the literal-zero rule

**Why the literal-zero rule could not be met, measured on round 21.** Of 38 findings, ~33 were
*tree drift* — a line number, count, or label true when written and rotted since (`:560`→`:593`,
"26 crates"→50, "3/23"→3/48, "-V 5/9"→6/9, "4 templates"→17) — and ~5 were *build-relevant*
(a done-signal `jq` that accepts `receipt:{}`, an `S3` `jq` that filters malformed rows before
validating, an `S4` done signal calling a `br dep check` that does not exist, an
`artifact_provenance` gate cited and absent). The plan carries **321 `file:line` citations and 105
bare tree counts** in prose against a tree committing ~2/min. Grading the live tree re-measures
them every round; convergence on that shape is unreachable by construction, and lowering the bar
would mean building from a plan nobody has agreed on. Command: `grep -ohE
'\b[A-Za-z0-9_./-]+\.(rs|md|toml|ts|go|jsonl):[0-9]+' docs/plan/[0-9]*.md | wc -l`.

**The rule, in five clauses:**

1. **A round grades a PIN, not the tree.** Every `CONVERGENCE.jsonl` row from round 22 on carries
   `pin: "<git sha>"`; the grader is dispatched with that sha and reads nothing newer. A fact true
   at the pin is not a finding. Drift after the pin is a re-pin, never a defect.
2. **One round at a time.** Round N+1 is not dispatched until round N's findings are fixed in
   place, the section files are committed, and the integrator (`%1397`) cuts the next pin. The
   pin is a commit; there is no "current" plan between pins.
3. **The drift class is killed once, mechanically, not re-graded.** `file:line` citations become
   construct names (PV8); bare tree counts route through `NUMBERS.toml` keys; a lint refuses new
   bare `:NNN` citations in plan prose, with fixtures in both directions.
4. **The convergence target is the build, not the prose.** `docs/plan/FOUNDATION.jsonl` (one row
   per stage S1–S9, per `SCHEMAS.toml [artifacts.journey_foundation]`, materialized from
   12-journey's existing F1–F5 + KNOWN/UNKNOWN/GAP blocks) plus the seven milestone OBSERVABLE
   blocks in 09-milestones §2 — **16 units** — are what must converge: two consecutive fresh-eyes
   rounds, two model families, **zero BLOCKER and zero MAJOR** against those 16. MINORs are
   recorded and do not block. Prose sections are re-checked only where a unit cites them.
5. **Then S4.** Beads materialize from FOUNDATION.jsonl under the S4 contract with the graph
   digest recorded, and every bead's ACCEPTANCE is one of the 16 units' done signals, verbatim.

**NO-CLAIM, unchanged:** severity is still a judgement by the graders; `graded_by` attribution
makes a downgrade attributable, not impossible. Convergence on 16 units proves the build is
*agreed*, not that it is *right* — M5 is still the milestone whose failure invalidates the thesis.

Rationale: on a 573 KB, 13-section technical document, a fresh reader will always
find *something* — prose that could be sharper, a figure that wants a citation. The
noise floor of a genuinely fresh reader is not zero and there is no evidence it ever
becomes zero. BLOCKER and MAJOR are claims about correctness and completeness;
MINOR is a claim about polish.

**NO-CLAIM, and it is the load-bearing one:** severity is a *judgement by the same
agents whose count we stopped trusting*. Nothing yet prevents a grader from
downgrading a real defect to MINOR to make a section bank — precisely the failure
mode "2 rounds of zero" had, one level up. The `schemas` gate forces every row to
CARRY a severity; it cannot check that the severity is HONEST. Two candidate
defences exist and neither is built: a held-out grader that only re-classifies
(never finds), and a rule that any MINOR touching a load-bearing figure or a claim
in `NUMBERS.toml` is automatically MAJOR.

**Nine of twelve sections have no round-13 row.** The three graded are the three
panes' current assignments. This amendment is a proposal recorded in the plan, not a
change to the contract — the contract still says zero findings, and by that contract
the board is **0/12 banked after 12 rounds**.

---

## Round 14 — the held-out lens, and it settles the stop-condition question

The withheld lens ran once across all 12 sections, per the rule reserved at round 10:
*"a task needs a held-out leg on a different distribution, to test that a fix
GENERALISES rather than fitting the graders."* Twelve fresh scouts, zero shared
context, one question each:

> It is 3am. An alarm woke you. You have never seen this project. You have ONE
> section and nothing else. **Can you ACT?**

Not *is it true* — that was graded thirteen times. **Can a stranger under pressure
do something.**

### Result

| lens | question asked | findings |
|---|---|---|
| rounds 1–13 (hillclimb + capability) | is this document CORRECT? | 15 BLOCKER, 40 MAJOR |
| **round 14 (held out)** | can an operator ACT? | **net 20 BLOCKED, 49 gaps** |
| verified false positives | — | 3, all one artifact class |

**Thirteen rounds of correctness grading left twenty net blocking actionability
defects, because no round asked.** That is what a held-out lens is for, and it is the
first time this plan has been measured on a distribution its graders never saw.

### The answer to the stop condition

Under the current contract — *"2 rounds of zero new findings"* — the correctness
lens was the only lens. Had it ever reached zero, this plan would have been
certified while a 3am operator could not invoke a single one of actions A1–A11,
could not name a valid `<adapter>` for a documented command, and could not locate
the artifact behind the plan's sole wire-level proof.

Convergence on one lens is not convergence. **The stop condition needs a lens
requirement, not only a count requirement** — and that is a change to the contract,
so it is Josh's to make, not mine.

### Three findings the lens produced that correctness could not

1. **`/tmp` provenance is plan-wide, not local.** `Lens10PriorArt` independently
   found nine Gap verdicts grounded on `/tmp/grade/` artifacts — the same clearable-
   provenance class already fixed in §1.2.3 and §4.7. Three sections, one defect,
   found only by someone asking "could I retrieve this at 3am".
2. **A dead citation I created an hour earlier.** `Lens01Idea`: the §1.2.1 table
   still pointed at `/tmp/grade/agent-end-raw-frame.json` while the durable path
   lived only in the §1.2.3 subsection *"I might skip, thinking it's already
   resolved."* Corrected in the same tick.
3. **`br comment` exits 2 and drops the comment.** The section claimed it, and the
   lens flagged it as an unverifiable operational claim. Measured directly:
   `br comment` → exit 2, comment absent; `br comments add` → exit 0, comment
   present. The plan was right and the operator would have been silently wrong.

### NO-CLAIM, and it is load-bearing

**The lens has a systematic false-positive mode.** Three BLOCKED findings claimed
truncated sentences; all three were **line-wrap artifacts** in the agents' reader —
a multi-line shell one-liner in §09 and a wrapped criterion in §06, both complete in
the file. I verified each rather than recording it, and the rows carry
`false_positives_verified` so the raw and net counts are both visible.

A held-out lens is a *different* reader, not a *better* one. Its output needs the
same verification as any worker report — which is exactly the discipline the rest of
this document already demands, applied to the newest instrument.

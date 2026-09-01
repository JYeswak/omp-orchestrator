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

### S5 — Execution

**Trigger.** The beads DAG (S4) contains a ready, unclaimed, non-epic bead, and a pane is ConfirmedIdle.

**Dispatch packet.** Bead id + WHAT/WHY/ACCEPTANCE verbatim from the bead body + the file reservation list (`ntm locks`) + the stage's packet-journal append. Dispatched only after `ntm claim <id>` succeeds — an unclaimed send is the `5rh`-to-`%1413` defect (11-lifecycle), measured twice.

**Amazing.** Every dispatch in the wave has: a claim row, a file reservation, a per-target receipt, and a packet-journal record — zero exceptions across a 10-dispatch wave, counted from the journal, not from memory.

**Adequate.** 1:1 dispatch with claim + receipt; fan-out to N panes done as N sequential 1:1 sends with the receipts collected by hand. Costs later: the fan-in barrier does not exist, so a partial wave reads as complete until a human notices (the cp-z42vu class at N scale).

**Negative patterns.** (1) Unclaimed dispatch — `5rh`-to-`%1413`, measured twice (11-lifecycle §S5). (2) Transport success ≠ delivery — `cp-z42vu`, `success:[4]`, packet never arrived (dispatch-silence-watch/src/lib.rs:10-11). (3) Recency over graph — 19 waves dispatched newest-first while PageRank named the articulation point (stand-down confession; live proof `bv --robot-next` → omp-orchestrator-2o5, "Unblocks 2z2.1/2z2.2", score 0.492).

**Skills.** `vibing-with-ntm` (pane coordination; does NOT cover claims/receipts — it predates them), `beads-north-star` (bead shape the packet carries), `multi-agent-swarm-workflow` (wave mechanics; assumes a single shared session, which is exactly the 1:many gap).

**Done signal.** The packet-journal row for this dispatch carries `receipt: RECEIPT_CONFIRMED` (or a typed refusal naming the target) and `claim_id`; command: `jq 'select(.bead=="<id>")' docs/plan/DISPATCH.jsonl | last` — exit 0 with a non-empty receipt object.

**F1 SCHEMA.** `DISPATCH.jsonl` — append-only. Required: `ts, wave, bead, targets[], transport, claim_id, receipt{verdict, evidence}, journal_seq`. Row already declared in SCHEMAS.toml as `DISPATCH.jsonl` (append-only; the S5 writer is the only allowed writer). SCHEMAS.toml row: EXISTS (`[artifacts.dispatch_journal]`, added this wave).

**F2 I/O CONTRACT.** Input produced by: S4 (the beads DAG — `br ready --json` filtered by loop-queue-filter) and the claim fence (dispatch-claim-fence `DispatchPermit`). Output consumed by: S6 (grading reads the journal's packet/receipt pair to know what to re-derive) and the reap path (dispatch-silence-watch keys on `assigned ∧ in_progress ∧ no-comment`). The receipt consumer is receiver-receipt (`assess_receiver_receipt`). No unnamed consumers.

**F3 CRATES.** Mechanism: `dispatch-claim-fence` (permit), `receiver-receipt` (verdict), `ack-stage` (transport types), `dispatch-silence-watch` (silence detection) — all exist. Thin caller: the dispatch step in `omp-orchestrator` (main.rs run path) — exists, currently a human types instead. MUST BE CREATED: nothing — the mechanism set is complete; the wire is the work.

**F4 GATES.** Gate: the dispatch claim fence refuses a packet naming an unclaimed bead, and the transport gate refuses a bare success without a receipt. Known-BAD leg (IN-TREE, per beads-north-star): `dispatch-silence-watch`'s cp-z42vu fixture is the planted specimen — a test that feeds `success:["4"]` with no arrival and asserts the verdict is NOT `Delivered`. Exists: yes (dispatch-silence-watch tests). The claim-fence's known-bad: the `Reassigned` arm test. Both in-tree. REFUSES: unclaimed send, receipt-less success, and (the one that does not exist yet) partial fan-in reported as complete.

**F5 NUMBERS.** Figures this stage claims, to be declared in NUMBERS.toml on first run: `dispatch_journal_rows` (baseline 0 today — declare with `expect="0"` and ratchet up; NUMBERS gate fails on drift, which IS the ratchet), `unclaimed_dispatches` (expect 0 after the claim wire; any nonzero is a regression), `fanout_partial_waves` (expect 0). Declared today: none — the stage has not run; declaring a number for a stage that has never executed is a figure with no derivation, which is the defect this field exists to kill.

**KNOWN.** 18-edge DAG complete and verified (round 10: scanner-identical); claim fence + dispatch fence built (fence held 4.2h); `ntm claim/locks/message` surfaces probed live; `AgentEndEvent` completion wire-proven `{"type":"agent_end","isTerminal":true}` on `RpcSessionEventFrame`; 162 refused ticks / 4.2h with `DISPATCH_RETRY_BLOCKED`.

**UNKNOWN.** (1) Does per-target receipt survive a multi-target `--robot-send`? Experiment: one 3-pane wave, compare per-target receipts against pane truth. Cost: one wave, ~10 min. (2) Does `ntm claim` hold across a pane restart? Experiment: claim, kill pane, respawn, re-check `ntm locks`. Cost: ~5 min. Both cheap; both run before the first bead of the first wave, per §12.10's cheapest-falsifier rule.

**GAP.** Fan-out/fan-in primitive (barrier + partial-verdict): cost of leaving it missing = every multi-pane wave is N hand-typed sends with hand-collected receipts, and a partial wave is indistinguishable from a complete one — the cp-z42vu class at scale. Packet journal: cost of leaving it missing = every forensic question ("which packet did this?") requires a human memory — measured: the reap could only name "seven conditions living in scrollback."

### S6 — Grading the work

**Trigger.** A dispatch receipt is `RECEIPT_CONFIRMED` and the worker asserts done — or the silence threshold fires (`FINDING_THRESHOLD == 3`).

**Dispatch packet.** The bead id, the claim's cited ACCEPTANCE commands, the packet-journal entry, and the instruction: re-derive every cited command; never read the worker's report as evidence. Sent to a pane that did NOT do the work (fresh eyes; grader rotation per the AAR held-out leg).

**Amazing.** Every close cites a re-run command whose transcript is stored on the bead, and the grader's pane id differs from the worker's — zero self-graded closes across a 10-bead window, counted from the journal.

**Adequate.** Spot-check grading: 1 in 3 closes re-derived fully, the rest checked for cite-presence only (the pre-delete-citation-check shape). Costs later: two-thirds of closes carry un-re-run evidence — the `cp-3k9jq` class (104-char close reason, zero path citations, three citations of a deleted script).

**Negative patterns.** (1) Worker-asserted done as the close condition — `ack-spine`'s own taxonomy classes `Finished` as a claim (followup.rs), and M4's fix requires the close actor ≠ dispatched worker. (2) Grading that can only pass or fail — bead ipg.17 was "built, correct, and undiscoverable": `Grade` needs a third arm (09 A8). (3) Prose close reasons — 29-bead wave, 8 gaps named in prose and never filed (finding/src/lib.rs:6-10).

**Skills.** `beads-compliance-and-completion-verification` (the audit shape; does NOT cover the receipt/claim chain), `verification-before-completion` (the re-derivation discipline), `beads-north-star` (VERDICT comment shape).

**Done signal.** The graded close carries: grader pane id ≠ worker pane id, ≥1 re-run command with stored transcript, and a `Grade` value from the shared type (once it exists — today the type is the gap). Command: `br show <id> --json | jq .close_reason` contains the re-run command AND the transcript path.

**F1 SCHEMA.** `Grade` — the largest missing type in the workspace (6 Verdict-shaped types, no shared trait). Required fields: `bead, verdict{PASS|FAIL|UNREACHABLE}, rerun_commands[{cmd, exit, transcript_path}], grader_pane, worker_pane, graded_at`. Row to be added to SCHEMAS.toml when the type lands (writer: the grading pane's harness). Until then S6 persists nothing of its own — it writes bead comments, which S9's ledger covers.

**F2 I/O CONTRACT.** Input produced by: S5 (the journal entry naming packet + receipt) and the worker's claim on the bead. Output consumed by: S7 (validation trusts only graded closes) and S9 (the decision ledger amortizes grading disputes). The consumer that makes this stage non-decorative: `br close --reason` refuses prose-only reasons (the finding-crate's 29-bead wave is the measured refusal).

**F3 CRATES.** Mechanism: `close-evidence-gate` (exists; grades evidence shape), `ack-spine` (exists; FollowUpVerdict, zero callers — occurrence 21 of built≠wired). Thin caller: the grading dispatch (omp-orchestrator). MUST BE CREATED: the `Grade` shared type — small, single-file, in omp-types (which exists and has zero dependents; this is the dependency that gives it one).

**F4 GATES.** Gate: close-evidence-gate refuses a close whose reason cites no re-runnable command or path — known-bad leg is the `cp-3k9jq` fixture (104-char reason, zero citations, measured). Second gate: state-wildcard-lint keeps the exhaustive-match property that makes `Grade`'s verdicts total. REFUSES: prose closes, self-graded closes, closes citing deleted paths (pre-delete-citation-check, `CitationConflict`, measured at `:28-37`).

**F5 NUMBERS.** Figures: `self_graded_closes` (expect 0; today: unmeasured — the close actor is not recorded, which is gap, not number), `cites_per_close` (floor-raise: ≥1 re-runnable cite; today unmeasured), `grade_type_dependents` (expect ≥1 once `Grade` lands; today 0 by census). Declared today: none — same rule as S5.

**KNOWN.** FollowUpVerdict::Finished declared (zero callers — occurrence 21 of the class); `FindingPriority` "P0"–"P3" exists upstream (`tools/review.d.ts`, DECLARED only) — the priority vocabulary for grading disputes already has an upstream shape; close-evidence-gate's citation-scan legs are measured (pre-delete-citation-check `:148,:162,:192-203`).

**UNKNOWN.** (1) Can `FindingPriority` upstream carry our grade disputes, or do the P0-P3 semantics mismatch? Experiment: map 10 real grading disputes from CONVERGENCE.jsonl onto P0-P3 and inspect the fit. Cost: ~1 hour, zero code. (2) Is `FollowUpVerdict` wired-able as the S6 trigger, or is its zero-caller status structural? Experiment: one pane emits a `Finished` claim through ack-spine's path and the journal records it end-to-end. Cost: ~half a day.

**GAP.** The `Grade` type: cost of leaving it missing = grading stays prose, grades cannot be counted/queried/required, and the twelve in_progress beads with no verdict stay invisible — measured tonight (the reap found exactly that). The S6→S7 receipt of grading (graded close → validation) has no consumer wired; cost = validation trusts ungraded closes.

### S7 — Validation

**Trigger.** A milestone's beads are graded-closed (S6) and the milestone's observable has never run on the target surface (fresh host, unattended window, or foreign repo).

**Dispatch packet.** The milestone's OBSERVABLE verbatim (from 09's template): the command, the expected machine-readable result, the exit code, and the environment contract ("no build cache", "foreign repo", "24-hour window"). Plus the failure instruction: run it, record everything, refuse nothing — validation records, it does not fix.

**Amazing.** The observable runs on a machine that has never built the workspace and the transcript is reproducible by a third party from the recording alone — the M6/M7 standard, executed rather than specified.

**Adequate.** Validation on THIS machine against a cleaned surface (`cargo clean -p`, fresh fixture repo). Costs later: host assumptions invisible until the first foreign host — the installer's `/Users/josh` fallback and compile-time roots are measured instances of exactly that debt.

**Negative patterns.** (1) Validation by the builder — the AAR held-out leg: the builder's machine carries the state that makes the test pass (11-lifecycle Hole 1). (2) A green suite over an empty scan — the census's 183-vacuous-invariants defect, in validation clothing. (3) The 4h19m undetected outage: validation that ran once and was never re-run — an unattended window without re-validation is a single data point wearing a process's clothes.

**Skills.** `verification-before-completion` (the re-run discipline), `testing-conformance-harnesses` (golden + conformance shapes; does NOT cover the cold-host logistics), `condition-based-waiting` (unattended-window assertions; no cold-host coverage).

**Done signal.** The validation transcript exists at a recorded path, contains the observable's command + output + exit code, and names the host — re-readable without asking us anything. Command: `jq '.observable, .exit_code, .host' <transcript>` returns all three, non-null.

**F1 SCHEMA.** `validation-transcript.json` — required: `observable_id, command, output_digest, exit_code, host{os,arch,hash}, started_at, duration, refusals_in_window[]`. SCHEMAS.toml row: TO BE ADDED when the first transcript exists (declaring it now would be a row for an artifact with no writer — F1's own refusal).

**F2 I/O CONTRACT.** Input produced by: S6 (graded closes prove the parts work) and 09 (the observable definition). Output consumed by: S8 (ship gates on a passed validation transcript — no transcript, no ship) and S9 (a failed validation is a human-decision trigger, not a silent retry). The consumer names are the teeth: ship without transcript = the installer-workmanship failure (identity unproven at install time), measured as the 23-commit stale supervisor.

**F3 CRATES.** Mechanism: `receiver-receipt` (arrival/verdict shapes), `plan-check` (§09 §4's PROJECTED validator — MUST BE CREATED, honestly named as such), `unexpected-stop-classifier` upstream (DECLARED only — classifies unexpected session stops, which is the unattended-window's alarm primitive). Thin caller: the validation runner (a mode of omp-orchestrator or the installer's --check). Existence check: `ls crates/ | grep -E 'plan-check|validat'` → none; both named as must-be-created.

**F4 GATES.** Gate: the validation-refusal gate — a transcript with `exit_code == 0` but a missing output digest is REFUSED (the timeout-is-not-a-verdict rule applied to transcripts). Known-BAD leg: a planted transcript with exit 0 and empty digest (in-tree fixture, `tests/` — not a patch harness). REFUSES: exit-0-over-nothing, host-unrecorded transcripts, and windows where a refusal class recurred ≥3 without escalation (`FINDING_THRESHOLD`).

**F5 NUMBERS.** Figures: `validation_transcripts` (expect 0 today; ratchet up), `unattended_window_hours` (expect 0; the 4h19m outage is a negative datapoint, recorded as history not as the figure), `refusal_classes_escalated` (expect 0). Declared today: none.

**KNOWN.** The observable template (09 §1) is doctrine; the four-milestone chain (M5/M6/M7) is specified; upstream `unexpected-stop-classifier.d.ts` exists (probed, DECLARED only); `FINDING_THRESHOLD == 3` measured at finding-dispatch/src/lib.rs:15.

**UNKNOWN.** (1) Does the orchestrator binary run on a clean machine at all (missing dylibs, HOME assumptions)? Experiment: `cargo clean -p omp-orchestrator && cargo build --release && ./target/release/omp-orchestrator --once --repo /tmp/fixture` in a worktree. Cost: ~30 min. (2) What does 24h unattended actually cost in refusals? Experiment: the M7 window with the escalation consumer wired. Cost: one day of wall clock + the finding-dispatch wire (half a day). (3) Does `unexpected-stop-classifier` match our `Liveness` taxonomy? Experiment: map its arms against `PaneState` — an afternoon, zero code, prior-art payoff pattern.

**GAP.** No validation transcript exists for ANY milestone: cost = every "it works" tonight is a claim, and M5-M7 are unfalsifiable until the first transcript lands. The cold-host path has never been attempted: cost = M6 is the milestone the plan cannot falsify, and the installer's known host-coupling defects are the reason it will fail on first attempt.

### S8 — Ship

**Trigger.** S7's transcript passes for the release candidate, and a consumer exists that is not this repo (M6's foreign host, or a second project's journey).

**Dispatch packet.** The release candidate tag, the four-way identity checklist (HEAD == build_id == --version == running process), the install target, and the rollback command. The installer's own contract, executed rather than described.

**Amazing.** A one-command install on a cold host that passes the four-way identity check at install time — the check that fires when HEAD ≠ build_id, measured as the 23-commit stale supervisor defect — plus a tested rollback to the prior binary.

**Adequate.** Manual install on this machine with the identity check run by hand. Costs later: no rollback path (the M6 standard degrades to "reinstall and hope"), and the identity check stays a convention — the exact gap that let a stale binary supervise the fleet for 4.2 hours.

**Negative patterns.** (1) Identity unproven at install — the 23-commit stale supervisor (README:155 family). (2) Host-coupled defaults — `/Users/josh` fallback at installer main.rs:25, compile-time roots at :16-20, measured. (3) Install-plane colonialism — 08 §3's table: conventions imposed on the adopter's repo.

**Skills.** `installer-workmanship` (the four-way identity, dual checksums, atomic lock, per-crate summary — the shape this crate already follows), `release-preparations` (version + checksums + transcript; GitHub-release assumptions do not transfer to a local-first tool), `rust-crates-publishing` (crates.io path; NOT this — the mission is local-first install).

**Done signal.** `installer --check` exits 0 on the installed host AND the rollback command has been executed once, returning the prior binary, with both transcripts stored. Command: `<installer> --check; echo $?` → 0, and the rollback transcript exists at the recorded path.

**F1 SCHEMA.** The installer already persists: install manifest (binary → path → sha → HEAD). Required fields measured from the crate: `binary, install_path, sha256, head_at_build, checked_at`. SCHEMAS.toml row: EXISTS in shape via the installer's own code; formal row TO BE ADDED when the manifest file path is pinned (currently printed, not persisted — a gap, recorded below).

**F2 I/O CONTRACT.** Input produced by: S7 (the passing validation transcript gates the ship) and the workspace build. Output consumed by: the foreign host's S1/S5 (the installed binary runs the next journey), and `--check` (the drift detector, consumed at every subsequent boot — the standing drift check Josh named). The rollback artifact is consumed by the operator, once, under failure.

**F3 CRATES.** Mechanism: `installer` (exists, isolated — on neither side of any of the 18 DAG edges, so nothing consumes it; this stage is the consumer that ends its isolation). Thin caller: the ship step of the journey (a tag + an installer invocation). MUST BE CREATED: nothing — the crate is the mechanism; the S8 wire is `installer --check` being called by something other than a human.

**F4 GATES.** Gate: the four-way identity check (HEAD == build_id == --version == running), which fires and names the drift — measured live: it caught the 96lacd/36fc41e mismatch and named all three identities. Known-BAD leg: the staged-file refusal (`installer --install` on a dirty tree refuses with named file — measured). REFUSES: stale binaries, unproven installs, and missing rollback artifacts.

**F5 NUMBERS.** Figures: `identity_check_exit` (expect 0 at ship; the drift measurement is the history), `install_coverage` (3 of 21 binaries tonight — declared in NUMBERS as installer_known_binaries=3), `rollback_tests` (expect ≥1 before ship; today 0). Declared today: installer_known_binaries (exists in NUMBERS).

**KNOWN.** installer crate built: four-way identity check fires on live drift and names all three identities (measured, `--check` round 7 wave); 3 identity tests green; `/Users/josh` fallback at :25 and compile-time roots at :16-20 measured; the crate is isolated in the DAG (nothing consumes it — 03-crates §3.4).

**UNKNOWN.** (1) Does the installer work on a machine without this repo? Experiment: the M6 cold-host transcript is the same experiment — cost shared with S7. (2) Does `cargo install omp-orchestrator` (08 §2.1's PROJECTED path) produce a binary whose four-way identity CAN pass, given compile-time roots? Experiment: install to a scratch CARGO_HOME and run --check. Cost: ~20 min. This is the cheapest falsifier for the whole ship stage and it has never been run.

**GAP.** The install manifest is printed, not persisted: cost = `--check` cannot compare against the record of WHAT was installed, only against HEAD — the third identity is weaker than it looks. The rollback path is untested: cost = ship is irreversible, which makes every ship a bet.

### S9 — Human requirements stored (cross-cutting)

**Trigger.** Any human intervention in any stage: a ruling, a correction, a priority call, a stand-down. The trigger fires the moment it happens — S9 is not a phase, it is a discipline the other eight stages call.

**Dispatch packet.** No pane dispatch. The packet is a RECORD: `{id, ts, question, decider:"Josh", decision, options_considered[], binds_stages[], supersedes[], review_after}` — one row per ruling, appended to the decision ledger, and a bead comment citing the row id (the bead is the durable anchor; the row is the queryable one).

**Amazing.** Every human decision in a stage's run is a ledger row before the stage closes, and amortization happened: rows with `binds_stages` were promoted into AGENTS.md/CLAUDE.md or expired into beads, counted per cycle ("promoted: N, expired: M"). Zero decisions living only in scrollback at cycle end.

**Adequate.** Decisions recorded as bead comments in the fixed shape, amortized weekly instead of per-cycle. Costs later: decisions are queryable only by br search, the binding scope is prose, and the review discipline decays — the bead-comment fate of tonight's eleven rulings if nothing changes.

**Negative patterns.** (1) Decisions in scrollback — eleven of Josh's rulings tonight, plus the reap's "seven real conditions" finding; measured twice, both fatal at compaction. (2) Agent-asserted "Josh said" without a record id — unfalsifiable, and this session's graders cannot cite it. (3) Decisions that bind forever — a rule nobody can challenge is the 60,467-line accretion in policy form.

**Skills.** `beads-north-star` (the audit-trail-in-bead doctrine — the anchor half), `jsm` (skill capture — the amortization target for repeated decisions), `cass`/`cass-memory` (session archaeology — the recovery path when the ledger misses one; recovery, not storage).

**Done signal.** `jq '.decider' docs/decisions.jsonl | sort | uniq -c` shows the deciders, and every row's `binds_stages` names real stages; the amortization count per cycle is nonzero or explicitly zero-with-reason. Command exits 0 with ≥1 row for the cycle.

**F1 SCHEMA.** `docs/decisions.jsonl` — the highest-value schema in this stage set. Required: `id (DEC-<n>), ts, question, decider, decision, options_considered[], binds_stages[], supersedes[decision-id], review_after, recorded_by`. SCHEMAS.toml row: **ADDED THIS WAVE** as `[artifacts.human_decisions]` (path `docs/decisions.jsonl`, format jsonl, writer marked unbuilt). The upstream neighborhood, checked first: `Stage1Claim`/`GlobalClaim` with `ownershipToken`/`inputWatermark` (memories/storage.d.ts:20-27) is memory-claim domain — does NOT transfer; `tools/approval.d.ts`'s `ToolApprovalDecision` + `ApprovalPolicy` ("allow"|"deny"|"prompt") and `ResolvedApproval` is the closest upstream shape (DECLARED only) and should be reused for the `decision` field's type rather than invented beside.

**F2 I/O CONTRACT.** Input produced by: any stage (S1-S8), by the human directly, or by an agent filing on the human's behalf (with `decider` attributed, never the agent). Output consumed by: the dispatch packet builder (binds_stages rows are attached to the packet), the AGENTS.md amortization pass (S9's own stage-close), and the grading pane (an unfalsifiable "Josh said" is refused; the row id is required). The consumer names make S9 load-bearing in both directions.

**F3 CRATES.** Mechanism: none exists — MUST BE CREATED, and it is deliberately tiny: a writer is a skill invocation (`ms`-style) or a 50-line Rust utility over JSONL; the storage is the repo. Thin caller: every stage. The orchestrator's packet builder is the first consumer. Honest alternative until it exists: bead comments in the fixed shape (durable, survives panes) — the adequate form of the same schema.

**F4 GATES.** Gate: the decision-ledger gate refuses (a) a row missing any required field, (b) a `supersedes` pointing at a nonexistent row, (c) a row whose `review_after` has passed without an amortization record. Known-BAD leg: a planted row with empty `decision` and a `supersedes` to DEC-999 — in-tree fixture in tests/, per beads-north-star. REFUSES: anonymous decisions, dangling supersessions, and unreviewed bindings.

**F5 NUMBERS.** Figures: `decisions_ledger_rows` (expect 0 today; ratchet up — declared NUMBERS row on first run), `decisions_in_scrollback` (expect 0 after the discipline lands; today 11, measured), `amortized_per_cycle` (ratchet). Declared today: none — zero-row baselines get declared when the first row lands, so the registry never holds a figure with no instance.

**KNOWN.** Eleven Josh rulings tonight in scrollback (measured by the reap and by this pane's transcript); the schema fields named by three-agent convergence (#1 unanimous); the upstream approval vocabulary exists (tools/approval.d.ts, probed); bead comments as the adequate substrate are doctrine (beads-north-star).

**UNKNOWN.** (1) Does the fixed shape survive contact with real rulings, or do decisions arrive as multi-part and overlapping? Experiment: backfill tonight's eleven rulings into the schema by hand and inspect the fit — cost: ~1 hour, zero code, and it seeds the ledger with real rows instead of zeros. (2) Who files when Josh is asleep and the ruling is implicit (a stand-down tone)? Experiment: wire the reap to propose decision rows from transcript deltas for Josh to confirm — cost: ~half a day; the proposal/confirm split keeps the human the decider.

**GAP.** No mechanism anywhere: cost = every ruling is one compaction from gone, every future agent re-briefs by hand (measured all session), and the plan's own §8 open questions accumulate answers nobody can query. The backfill experiment (UNKNOWN 1) is the cheapest falsifier for the whole stage and it runs tonight.

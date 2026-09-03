# Planning to Exhaustion — the S0 phase, and why a big plan never gets planned fully

**Bead:** `omp-orchestrator-planning-to-exhaustion-PX` (filed with this document)

## Purpose

Joshua, 2026-09-03: *"how can we package this whole s0 / s1 process — plan each section through to exhaustion before executing, then measure to exhaustion, then plan next phase… jeff does a lot of planning and has his entire plan worked through to completion before he builds — i dont know how he does it honestly because every time i try to send such as massive plan to agents it NEVER — not once — gets planned fully."*

This document answers the second half first, because the answer is the mechanism.

## `PX-1` The diagnosis: "fully" is a quality, and agents optimise for the appearance of a quality

**"Plan this fully" has no falsifiable stop condition, so an agent stops when it feels done.** "MISSING = 0 across 214 enumerated rows" has one, and an agent cannot feel its way past it. Every measured failure below is a variant of asking for a quality and receiving its appearance.

Measured in one session, all in this repo:

| `PX-ID` | what happened | the quantity that would have prevented it |
|---|---|---|
`PX-D1` | **wave 1: 88 disagreement rows across 12 boxes, 0 reviewer rejections.** A survey wearing a review's clothes | *rejections* is a number; a wave with 0 is a failed wave |
`PX-D2` | wave 2, same fleet, explicit attack instructions: **25 rejections in 50 rows, 5 refutations that changed the box** | the instruction changed, not the agents |
`PX-D3` | `s1_l1_doctor.md` + `s1_l2_ecosystem.md`: **24 KB of correct reasoning, 0 stable IDs, 0 named tests.** A machine cannot reference it, so coverage against it is unmeasurable | *stable_ids* is a number |
`PX-D4` | `s1_l0_install.md`: **25 stable IDs, 6 laws, 0 named tests.** Nothing said what proves each law | *named_tests per law* is a number |
`PX-D5` | **9 of 10 `[[box.gap]]` rows uncovered**; 3 cited beads (`plf.7.3`, `lf.7.12`, `plf.7.2`) that do not exist — the leading `j` was dropped | *resolvable citations* is a number |
`PX-D6` | I flipped S1 to `converged` on **zero-open-rows** while five layers read `exists = "none"` | zero open rows is necessary, not sufficient |
`PX-D7` | I published `rows = 62`; a fresh clone of the same sha measures **17** — 35 rows were untracked | every figure labelled TREE or WORKTREE |
`PX-D8` | **18 crates and 21 type collisions landed in ONE DAY** while a test asserted a hardcoded set of 6 | a figure carries its producing command, never its value |
`PX-D9` | a bead can be filed with **no acceptance criteria** and nothing refuses it, so *planned* and *listed* are indistinguishable | acceptance is a required field |

**`PX-2` The load-bearing sentence: a plan is exhausted when its MISSING count is zero, and it cannot be exhausted before someone produces the denominator.** Until the denominator exists, "is this planned?" is a matter of opinion, and opinion is what agents supply on request.

## `PX-3` How the corpus actually does it — three mechanisms, none of them discipline

Read from the repo rather than inferred:

- **`PX-3a` The plan is rows with stable IDs, not prose.** `crate-atom-gate` makes "done" a **nine-part checklist per crate** (`crates/crate-atom-gate/src/lib.rs:51-70`): lib, bin, verdict, tests, fuzz, claim, slo, oracle, wired-caller. 68 crates × 9 parts = **612 rows**, of which **~500 are MISSING** — and that number is printed, not felt. Completeness is a count.
- **`PX-3b` Every figure resolves to a runner.** `NUMBERS.toml` exists because five grading rounds produced the same defect: a number correct when written and wrong now. Its rows are `[figures.*]` with `command` / `expect` / `appears` / `note`. A plan whose numbers carry commands survives its own drift.
- **`PX-3c` Exceptions are named rows with an owner and an expiry, never silence.** `UNWIRED_LANE_ALLOWANCE: &[(&str, &str)] = &[]` — an **empty allowlist**, and `SystemicAllowance` carries an `owner` and a `dies_when`. The ceiling only shrinks.

The pattern: **turn the plan into a denominator, publish the MISSING count, and drive it to zero.** Not "plan harder".

## `PX-4` The S0 phase machine

Six phases. Each has a **machine-checkable exit predicate**; you may not enter the next until the predicate holds.

```
P0 ENUMERATE → P1 ATTACK → P2 BEAD → P3 GATE → P4 EXECUTE → P5 MEASURE → (S+1) P0
```

| phase | output | exit predicate (the number) |
|---|---|---|
`PX-P0` ENUMERATE | one row per thing to BUILD and one per thing to TEST, each with a stable ID | `MISSING = 0` in the coverage matrix; every row addressable by ID |
`PX-P1` ATTACK | non-author rows against the enumeration | `refutations ≥ 1` per wave from a non-owner; a wave of all-accepted rows is FAILED and reruns with a different reviewer |
`PX-P2` BEAD | one bead per row, acceptance in run-X-expect-Y form | `beads_filed = rows`, and `beads_without_acceptance = 0` |
`PX-P3` GATE | every bead wired to its layer/stage gate | `br dep cycles` empty; the stage gate's tree contains only that stage |
`PX-P4` EXECUTE | build waves, layer-chained L0→L5 | each layer gate closes only on contract + wired caller + observability writer + gate-fired-on-known-bad + non-author grade |
`PX-P5` MEASURE | expected vs measured per row | `UNMEASURABLE = 0`; a stage exceeding its SLO is RED **only if an expected row exists** — otherwise UNMEASURABLE, never green |

**`PX-5` The gate between phases is the point.** S2's `P0` cannot start until S1's stage gate closes, and the stage gate cannot close while `MISSING > 0`. That is what makes "plan to exhaustion, then execute, then measure to exhaustion, then plan next" a machine rather than an intention.

## `PX-6` Why the massive-plan dispatch fails, specifically

Four independent causes, each with its counter-move. All four were observed this session.

| `PX-ID` | cause | counter-move | evidence |
|---|---|---|---|
`PX-6a` | **no termination predicate** — "fully" cannot be checked | acceptance is a COUNT the agent must print (`L0_BUILD_ROWS=n L0_TEST_ROWS=n BEADS_FILED=n`) | every dispatch that produced complete work carried a count or a `file:line` to attack; the vague ones produced prose |
`PX-6b` | **no denominator** — coverage against an unenumerated plan is unmeasurable | `P0` produces the denominator BEFORE any bead is filed | the S1 readiness question was unanswerable until 63 IDs + 15 tests + 10 gaps + 24 observability + 96 hook fields were extracted |
`PX-6c` | **one agent cannot hold a massive plan** — the result is uniform shallowness | decompose by SECTION with disjoint file ownership; 3 agents × 2 layers | six contracts, 10-18 KB each; three panes produced depth AND two of them refuted each other. One agent over all six would have produced neither |
`PX-6d` | **agents converge instead of covering** — agreement is cheaper than enumeration | `P1` requires a refutation to advance | `PX-D1` vs `PX-D2`: same fleet, 0 → 25 rejections, on instruction alone |

**`PX-7` The corollary that explains Joshua's whole experience.** A massive plan sent as prose asks for `PX-6a` (a quality), against no `PX-6b` (denominator), from a single agent (`PX-6c`), with no adversarial requirement (`PX-6d`). All four failure modes fire at once, and the output *looks* like a plan — which is why it has never once been planned fully. The fix is not a bigger prompt. It is to send **one section, with a denominator and a count to print.**

## Contract Artifacts

- **Coverage matrix (TARGET):** `docs/plan/flow/S1-COVERAGE.md`, generated, one row per requirement with `bead_id | MISSING`. **DOES NOT EXIST YET** — it is the current `P0` deliverable, in flight on three panes.
- **Invariant suite (TARGET):** `crates/coverage-matrix/tests/coverage.rs` with the five named legs. Frozen.
- **Enforced today:** `PX-P1` by `CONTRACT.md:82-113`; `PX-P3` by the stage/layer gate DAG (`br dep cycles` empty); `PX-P2` acceptance-required by nothing — that is `DP-GAP-1`.

## Validation

```
cd /Users/josh/Developer/omp-orchestrator && \
printf 'PX ids=%s phases=%s gates_wired=%s cycles=%s\n' \
  "$(grep -c '^`PX-' docs/contracts/planning_to_exhaustion.md)" \
  "$(grep -c '^`PX-P' docs/contracts/planning_to_exhaustion.md)" \
  "$(br dep tree omp-orchestrator-gate-s1-djn8 2>/dev/null | wc -l | tr -d ' ')" \
  "$(br dep cycles 2>&1 | grep -c 'No dependency cycles')"
```

## Cross-References

- `docs/contracts/dispatch_preflight.md` — the ten dispatch stages and the twelve escape routes; `PX-P4` consumes it.
- `docs/plan/flow/CONTRACT.md:82-113` — the adversarial bar that enforces `PX-P1`.
- `crates/crate-atom-gate/src/lib.rs:51-70,115-121` — the nine-part denominator, `PX-3a`.
- `NUMBERS.toml` — `[figures.*]` with `command`; `PX-3b`.
- `docs/plan/flow/boxes/S1.toml` — `[box.contracts]`, `[[box.observability]]`, `[box.agreement]` with rows/rejections/refutations.

## NO-CLAIM

The phase machine is **enforced at two of six predicates**. `PX-P1` has a written bar and `PX-P3` has an acyclic gate DAG; `PX-P0`, `PX-P2`, `PX-P4` and `PX-P5` are checked by an orchestrator remembering, and `PX-P2`'s acceptance-required rule has no refusal at all. The gate DAG currently orders **15 beads** — the only ones without legacy edges — while 321 others are unattached, so `PX-P3`'s predicate holds over a nearly empty set. And this document is itself a plan: by its own `PX-2` it is not exhausted until its coverage matrix exists and reads MISSING = 0, which it does not.

## `PX-DONE` What DONE looks like for the planning phase

Six predicates. **All six, machine-checked.** No agent's opinion appears in the list, which is the point.

| `PX-ID` | predicate | checked by |
|---|---|---|
`PX-DONE-1` | `MISSING = 0` in the coverage matrix | `docs/plan/flow/S1-COVERAGE.md` prints `S1_REQUIREMENTS / COVERED / MISSING / DOC_ONLY`, generated by command, every count TREE-labelled. **First measurement, 2026-09-03: `401 / 135 / 266 / 0`.** |
`PX-DONE-2` | every requirement row has a bead **carrying acceptance** | `beads_without_acceptance = 0`. Measured on the first 68 filed: **68/68 carried acceptance**, median 422 B |
`PX-DONE-3` | every bead wired to its layer/stage gate | `br dep cycles` empty **and** `br dep tree <stage gate>` contains only that stage |
`PX-DONE-4` | **saturation**, not consensus — `PX-ROTATE` | one full lap with `new_findings = 0`, preceded by a lap with `new_findings > 0` |
`PX-DONE-5` | every contract addressable, every law provable | `stable_ids > 0` per contract **and** `laws_without_a_named_test = 0` |
`PX-DONE-6` | the **denominator itself** attacked | a non-author hunted for a further source and reported FOUND or NOT-FOUND *with where it looked* |

`PX-DONE-6` is the one people skip, and it has already paid: I asserted six sources were the complete denominator; pane 4 attacked it and found **four more** — `layer.exists`, `branch.test`, `decisions.HD`, `crate-atom.L0`. Had it accepted my six, `MISSING` would have read lower and been **a lie with a green number on it**, which is strictly worse than a large honest count because a reader stops looking.

## `PX-STAMP` How it is stamped in, so nobody can ask to start building

**The chokepoint already exists and demonstrably works.** `.git/hooks/pre-commit` runs eight gates and prints `CLEAN: all staged files passed the multi-gate checks` or `MULTI-GATE REFUSED: <n> violation(s)`. It refused this orchestrator twice today — `OC-L2` for dispatching before claiming, `OC-L1` for a malformed tick row. It is the only surface in this repo measured to have changed an agent's behaviour rather than described it.

So the stamp is a ninth gate, not a policy:

```
planning-stamp: REFUSE any commit that ADDS a directory under crates/
                while docs/plan/flow/S1-COVERAGE.md reports MISSING > 0,
                is absent, or does not parse.
```

- **known-bad leg (mandatory):** stage a new `crates/<x>/Cargo.toml` while `MISSING > 0` → REFUSED, and the refusal prints the current `MISSING`. A gate that has never fired on a bad input is not evidence.
- **known-good leg (mandatory):** docs-only and bead-only commits pass unchanged; a new crate passes once `MISSING = 0`. An attack-only suite ships an over-strict gate, and an over-strict gate gets routed around.
- **anti-vacuity, load-bearing:** an **absent or unparseable** matrix is an **ERROR, never a pass** — otherwise deleting the file unlocks the build, the cheapest bypass available to anyone who reads the gate.
- **scope:** it gates *adding* a crate, not editing one. Fixing `installer` or `loop-driver` stays possible while planning is open.

**Why a gate and not an instruction:** every social channel here has been measured silent. `ATTENTION.txt` took 178 consecutive writes with zero readers; a typed refusal naming `owner=josh` printed 29 times unread; six red CI runs went unread in one evening. An agent asking permission is a social channel. **A commit refusal is the only channel an agent cannot proceed past.**

## `PX-ROTATE` The round-robin — adopted, with one bug fixed

Joshua's shape is right: pane 1 analyses and gap-hunts, hands to 2, then 3, then 4, rotating.

**The bug is the stop condition.** *"Only once all agents say done"* is a **vote**, and a vote is the cheapest artifact a fleet can manufacture. Measured here: wave 1 produced **88 disagreement rows across 12 boxes with ZERO rejections** — four reviewers, unanimous, and the box was not reviewed at all. A protocol that terminates on agreement terminates early every time, and *feels* rigorous while doing it.

**Replace the vote with a saturation measurement:**

```
lap = pane1 -> pane2 -> pane3 -> pane4, each attacking a DECLARED DISTINCT AXIS
phase ends iff new_findings(this lap) == 0  AND  new_findings(some earlier lap) > 0
```

- **A first lap that finds nothing is a FAILED lap, not convergence.** It means nobody attacked. Rerun with different axes or different panes, exactly as `CONTRACT.md` clause (b) reruns a wave of all-accepted rows.
- **Distinct axes, or rotation is 4× the same read** and four agents converge on one blind spot. The axes: **(a)** denominator completeness — is a source missing; **(b)** acceptance falsifiability — can a grader actually re-run each bead's acceptance; **(c)** known-bad legs — does every gate and test row name the input that makes it RED; **(d)** citation resolvability and TREE-vs-WORKTREE labelling. Each pane declares its axis before starting and may not silently swap.
- **Findings are ROWS, never prose.** A refutation filed as a paragraph is invisible to the counter — measured: a real non-owner refutation sat in a contract's prose where `grep -c '^resolution = "refuted'` could not see it.

**`PX-ROTATE-BOUND` — is this too much?** No, but *unbounded* rotation would be. **Cap it at four laps.** If lap 4 still yields new findings, the conclusion is that the **denominator is wrong**, not that a fifth lap is needed — that escalates to Joshua as a P0, because a plan which keeps growing under attack has sections drawn wrong, and more laps cannot fix a bad decomposition. Cost, measured not estimated: one lap is roughly what three panes just did on the L0-L5 enumeration — ~10 minutes each, 68 beads, all 68 with acceptance. Four laps is cheap against discovering the denominator was wrong after a build wave.

**`PX-ROTATE-SEQ` the "are we sure" pass, after saturation.** Joshua's sequential phase is right and it is **not a second review — it is a re-execution.** Each pane re-runs, in order, the `## Validation` block of the artifacts it does **not** own, and pastes the output. That converts "we all agree" into "four agents independently re-ran each other's commands and got the same numbers", which is the only agreement worth having. Exit predicate: `commands_rerun = commands_declared`, zero output disagreements; any disagreement is a P0 row, not a discussion.

**`PX-E14` — the reserved escape-route row, now filled.** The first coverage matrix was generated by `python3 .git/s1_cov.py`. `.git/` is untracked by construction, so a `.py` generator placed there is **invisible to the gate that walks `git ls-files`** — the no-shell rule satisfied by geography rather than by compliance. Pane 4 disclosed it honestly and named the cause (the freeze blocks the Rust crate that should own it), which is why this is a recorded gap and not a deception. The consequence is real: the matrix is **unreproducible on a fresh clone**, so `PX-DONE-1` currently rests on an artifact only this working copy can regenerate.

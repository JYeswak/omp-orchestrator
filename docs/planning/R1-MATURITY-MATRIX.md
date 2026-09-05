# R1 maturity matrix — re-derived, not inherited

Measured 2026-09-05 at HEAD `c17585a` by `AtlasArcPass1` (atlas-arc loop pass 1 of 10).
Read-only toward `crates/**`; nothing under `docs/plan/**` was edited by this pass.

**Instrument boundary, stated first.** `~/.claude/skills/atlas-arc/` contains
`SKILL.md` + `references/` only — there is no `scripts/` and no `arc.py`, so
`arc.py init|lint|status|certify-plan` **cannot run** and `BUILD_READY` is **UNRUN, not
passed**. The sibling oracle `~/.claude/skills/planning-arc/scripts/validate_plan.py`
has zero occurrences of `maturity` (positive control: `grep -c 'def '` → 19), and
`grep -rl maturity crates/ --include=*.rs` → 0 files (positive control:
`grep -rl 'fn main'` → 94). **No executable oracle for R1 exists on this host**; every
score below is hand-measured against the rule text and carries a `file:line`.

## Rubric — stated before scoring

| level | name | required, and how it is discriminated here |
|---|---|---|
| 0 | MAPPED | addressable record with an identity, a name/purpose, its place in the topology, and a named owner |
| 1 | GROUNDED | ≥1 grounding row separating observation from intent: `claim` + the `command` that produced it + `value` + `measured_at` + `by`, and a current-reality status measured against source (`wired`/`exists-no-caller`/`MISSING`) |
| 2 | CONTRACTED | typed input, typed output naming a Verdict-shaped type, typed event row, named validator, and `branches` enumerating the verdict type's arms verbatim (the row shape at `docs/plan/flow/CONTRACT.md:13-33`) |
| 3 | FALSIFIABLE | one of the subject's **own** validators is shown able to bite: a `known_bad` leg as a first-class field, or a recorded observation of the validator refusing / going RED with the command. A known-bad leg named inside a `resolves =` gap **does not count** — that is the absence, not the falsifier |
| 4 | COMPILABLE | decomposable into schedulable work without redesign: a per-sub-unit contract set, per-sub-unit observability rows with reason-code sets and metrics, hook rows with `fail_mode` + `harm_class` + gauntlet stage, and named owning gates |
| 5 | FROZEN | `agreement.status = "converged"` **and** `approval = "HD-00NN"` **and** the four-clause bar at `CONTRACT.md:247-257` (rows > 0, rejections ≥ 1, refutations ≥ 1 from a non-owner, owner refutes) **and** the functional floor (no sub-unit `exists = "none"`) |

`SKILL.md:266` — "Do not label a section FROZEN because it is lengthy."

## Population — and why the verdict depends on it

R1's text (`SKILL.md:59-67`) names **no population**. This repo's own binding
measurement (`CONTRACT.md:140-145`, and the `HD-0014` body at `docs/decisions.jsonl:19-20`)
uses the twelve stage boxes as the section set. Three populations, three verdicts:

| population | n | max | median | delta | R1 |
|---|---|---|---|---|---|
| **12 stage boxes** — the set `HD-0014` actually used | 12 | 4 | 2 | **2** | **VIOLATED** |
| 13 numbered sections `docs/plan/00..12` | 13 | 3 | 3 | 0 | satisfied |
| combined, 25 subjects | 25 | 4 | 3 | 1 | satisfied |

An invariant whose denominator is unpinned is not an invariant → filed as
`omp-orchestrator-r1-no-checker-d81g`. **The primary verdict is the declared
population: `max=4 median=2 delta=2 VIOLATED`** — arithmetically identical to the
2026-09-03 figure, re-derived independently.

## Boxes — `docs/plan/flow/boxes/*.toml`

| box | M | evidence for the level reached | what blocks the next level |
|---|---|---|---|
| S1 | **4** | id `S1.toml:10-12`; grounding `:252-257`; typed `:19-22` + branches `:227` (20 arms); falsifier — six first-class `known_bad =` fields at `:393,:404,:415,:426,:437,:448` and an observed bite at `:116` ("refused this orchestrator twice on 2026-09-03"); compilable — `[box.contracts]` `:350-356` (six per-layer contracts with byte sizes and landing shas), six `[[box.observability]]` rows `:385-448`, eight `[[box.hook]]` rows `:102-211`, six named layer gates at `CONTRACT.md:104-107` | 5: `status="draft"` `:338`, `approval=""` `:339`, and the functional floor fails — `grep -c 'exists = "none"'` → 4 at `:41,:57,:72` plus partials `:49`, `:65` |
| S2 | **3** | id `:6-7`; grounding `:48-53`, `:130-142`; typed `:13-16` + branches `:21-28`; falsifier — an **observed refusal with its command** at `:24` and `:30-33`, plus `assembly_freshness` RED at `:25`/`:38` | 4: no contracts block, 0 layers, 0 observability rows; the single hook row `:145-155` says at `:154` it has "NO certified row of its own" |
| S3 | 2 | grounding `:55-85`, incl. `:83-85` "no crate implements S3 / MISSING"; typed `:13-16`; branches 6 | 3: the only known-bad mention is inside a `resolves =` gap, `:125` |
| S4 | 2 | grounding `:58-88`; typed `:15-16`, validator `:16` "MISSING as code — `ls crates/bead-lint` -> MISSING"; branches 8 | 3: known-bad only in `resolves =`, `:132` |
| S5a | 2 | grounding `:36-66` (five timestamped rows with commands); typed; branches 15 `:64-66` | 3: zero `known_bad`, no recorded refusal |
| S5b | 2 | grounding `:41-71`; branches 20, verbatim `ClaimFenceError::*` / `PacketError::*` arms `:23-33`; production caller at `main.rs:L1421` `:41-43` | 3: `:109` states the mutation leg is `MISSING` — "add a caller mutation leg … and must go RED". Nothing would go RED today |
| S6a | 2 | grounding `:34-64`, arms counted from source; branches 13 | 3: `:62-64` `lifecycle-arrow-gate` "MISSING; command returned no executable" |
| S6b | 2 | grounding `:29-59`; branches 8 | 3: same, `:57-59` |
| S6c | 2 | grounding `:34-64`; branches 14 | 3: same, `:62-64` |
| S7 | 2 | grounding `:42-79` (eight lanes, closed arm sets); branches 20 | 3: known-bad only in `resolves =`, `:134` |
| S8 | 2 | grounding `:38-89`; branches 16; **repaired row** `:12` now reads "rollback remains unproven until a retained transcript and persisted manifest exist", agreeing with `DESIGN_INDEX.md:129` | 3: `:73-75` "tests_dir=MISSING; no persisted rollback transcript was found" |
| S9 | 2 | grounding `:29-59`; branches 8 | 3: `:57-59` "decision gate and consumer references in production source / 0 matches" |

Sorted: `2×10, 3, 4` → **max 4, median 2, delta 2**.

## Numbered sections — `docs/plan/00-brief.md` … `12-journey.md`

Thirteen files, not twelve: `ls docs/plan/[0-9][0-9]-*.md | wc -l` → 13. The same
off-by-count was already corrected once inside this corpus at
`boxes/S2.toml:131-133` (18 files vs 13 sections).

| section | M | evidence | blocker |
|---|---|---|---|
| 00-brief | 3 | R1–R13 `:21-80` (the authority `DESIGN_INDEX.md:143` points at); writing contract `:565`; cross-section authority `:586`; kill criteria §8 `:774`; `:127` R6 OPEN "must produce a named G1–G8 proof artifact with known-bad" | 4: bead refs = 0 |
| 01-idea | 3 | viability gates `:403`, rows `:415-424`; falsification test `:80`; `:215` "would become unfalsifiable" | 4: bead refs = 0 |
| 02-surface-census | 3 | provenance contract `:3`; `INV-2026` `:14`; anti-vacuity gate spec `:349-359`; 16 MEASURED, 14 NO-CLAIM | 4: bead refs = 0 |
| 03-crates | 3 | crate table `:49`; per-gate refusal contracts `:78-85`; NO-CLAIM `:43`; PROJECTED lint `:93` | 4: 3 bead refs, none owning a claim |
| 04-diagrams | **2** | six numbered diagrams `:39,:133,:182,:232,:311,:354`, each MEASURED or PROJECTED with its extraction command | 3: `falsifi`=0 and `kill criteri`=0; the three NO-CLAIM rows are do-not-claim boundaries, not falsifiers |
| 05-actions | 3 | the section contract is `:1` "the negative pattern it must refuse"; `:8-9` marks each pattern scar-backed vs hypothetical; 21 MEASURED; six gate properties `:385` | 4: 1 bead ref |
| 06-gates | 3 | eight gate rows `:31-38`; six required properties `:227-234`; §2.1 fires-on-known-bad `:84`; §5 "What would make this whole framework fail" `:380`; 27 MEASURED | 4: bead refs = 0 |
| 07-installability | 3 | refusal-exit rule `:97`; negative patterns `:132`, `:145`; canonical manifest PROJECTED `:45`; NO-CLAIM `:104` | 4: bead refs = 0 |
| 08-end-users | 3 | require/never-require §3 `:260`; degradation ladder `:329`; abandonment criteria §7 `:398`; PROJECTED boundary `:7`, `:53`, `:59` | 4: bead refs = 0 |
| 09-milestones | 3 | done-definition template `:7`; M1–M7 `:45,:76,:95,:119,:142,:163,:215` each with STARTING POINT MEASURED; §5 "how to fail us" `:293`; §4 self-check `:267` | 4: bead refs = 0 |
| 10-prior-art | 3 | measurement boundaries §0 `:9`; instrument table with commands `:36-40`; four false-zero mechanisms `:42-46`; 21 NO-CLAIM | 4: bead refs = 0 |
| 11-lifecycle | 3 | canonical stage graph `:37`; five-property matrix `:74`; template-omission refusal tests with commands `:139-140`; cardinality contract `:445` | 4: 1 bead ref |
| 12-journey | 3 | runbook contract `:40`; `:250` "no known-bad leg → the gate is decorative"; `:279` cheapest falsifier; acceptance census `:303` | 4: 1 bead ref |

Sorted: `2, 3×12` → **max 3, median 3, delta 0**.

Two clean cross-section measurements, each agreed by `grep -oE` and a second regex
engine:

- **`non-goal` appears 0 times in all 13 sections.** Non-goals are a Charter
  requirement (`SKILL.md:50`) and an R5 fresh-agent obligation (`SKILL.md:132`). They
  exist only in the derived records: `no_claim` in 12/12 boxes, `non_goals` in 12/12
  `maturity0/*.toml`. → `omp-orchestrator-charter-surface-not-machine-visible-qwyr`.
- **Bead references: 6 occurrences across 4 of 13 sections; 9 sections carry zero.**
  This is the single discriminator holding every section at ≤ 3.

## Exception census

R1 (`SKILL.md:69-75`) requires a `CRITICAL_PATH_EXCEPTION` with reason, affected
sections, expiry condition, reviewer, and risks introduced.

`grep -rl CRITICAL_PATH_EXCEPTION` → three files, **every hit a negative**:
`CONTRACT.md:147` ("No `CRITICAL_PATH_EXCEPTION` is recorded"), the `HD-0014` decision
body, and `.skill-loop-progress.md`. Positive control:
`grep -rl "breadth before depth" docs/` → hits `CONTRACT.md`.

**EXCEPTIONS_FOUND = 0. EXCEPTIONS_WELLFORMED = 0.** Best candidate-equivalent
`HD-0014` (`docs/decisions.jsonl:19-20`) scores **3 of 5**: reason ✓, affected sections
✓, reviewer ✓ (`decider":"Joshua"`); **expiry condition ✗ as a field** — `review_after`
is `""` and there is no `condition` key, though `HD-0001`–`HD-0008` all carry one, so
the schema supports it; **risks introduced ✗** — the row's NO-CLAIM says what adoption
does *not* do, and no row states the risk of continuing at delta 2 (`HD-0003` carries
`residual_risk_stated_at_decision_time`; `HD-0014` does not). `HD-0018`
(`:25-26`) is worse on the same axis and records `decider":"measurement"`, which is not
a reviewer identity. → `omp-orchestrator-r1-exception-missing-so9x`.

`CONTRACT.md:147-152` argues the exception is unnecessary because the *ordering* it
would except is superseded. That disposes of the order, not of the delta; R1's
arithmetic is unconditional.

## Truth-surface census — `.atlas-arc/` NOT created

`.atlas-arc/` does not exist and was deliberately not created: that is a canonical
mapping change under the BUILD FREEZE at `CONTRACT.md:54-58`. **0 of 6 exist by name;
4 of 6 have their function served.**

| required surface | served by | machine-visible |
|---|---|---|
| `manifest.json` | **nothing.** `SKILL.md:738-742` wants plan hash + section maturity vector + open P0/P1 + blocking unknowns + ready frontier as one object. `METRICS.toml`, `NUMBERS.toml`, `SCHEMAS.toml`, `OMP-SURFACE-MAP.toml` are partials; none carries a maturity vector | — |
| `PROJECT_CHARTER.md` | `docs/plan/00-brief.md:16-80` + `docs/plan/R24-SOTA-CHARTER.md` | no — 0 stable IDs in the file |
| `PLAN_INDEX.md` | `docs/planning/DESIGN_INDEX.md` (self-declared index authority, `:9-12`) | yes |
| `IMPLEMENTATION_STATUS.md` | `docs/plan/CURRENT-REALITY.md` | yes, and **stale** (below) |
| `sections/S01.json …` | `docs/plan/flow/boxes/S1..S9.toml` (12) + `docs/plan/flow/maturity0/S1..S9.toml` (12) | yes (TOML, not JSON) |
| `registries/*.jsonl` | **4 of the 13 named kinds**: decisions → `docs/decisions.jsonl`; unknowns → `docs/plan/flow/unknowns/{DEFECTS,DISPOSITIONS}.toml` + `CENSUS.json`; beads → `.beads/issues.jsonl`; defects → `unknowns/DEFECTS.toml` (16 `DEF-*`) + `docs/planning/round{3,4,6a,6b,6c,6d}/DEFECTS.toml`. **No carrier** for requirements, interfaces, invariants, claims, risks, gates, workstreams, rigor_bindings, receipts — every filesystem hit for those names is inside `./.rch-tmp/` vendor caches. `registries/` at the repo root holds one file, `allowances.toml`; `boxes/S2.toml:97` independently names "registries/claims.toml row" as a **gap** | partial |

## Four governing surfaces — `GOVERNING_SURFACES_VISIBLE = 1/4`

| surface | carrier | machine-visible |
|---|---|---|
| Charter | `00-brief.md:16-80`, `R24-SOTA-CHARTER.md` | no |
| Target-state plan | `docs/PLAN.md`, `docs/plan/00..12`, the twelve boxes | no single carrier |
| Current reality | `docs/plan/CURRENT-REALITY.md` | yes — the only one, and it carries the explicit four-row table at `:14-21` that this section is checking against |
| Execution state | `.beads/issues.jsonl` (791 rows) | yes |

### Conflations found

1. **`CURRENT-REALITY.md` — the only machine-visible four-surface artifact is a
   present-tense snapshot.** `:6` pins `Repository_HEAD 05cdd2a`;
   `git rev-list --count 05cdd2a..HEAD` → **25**. `:21` asserts "761 parsed rows;
   closed=104, open=527, in_progress=95, grading=11, tombstone=22, blocked=2";
   re-measured now: **791 rows, closed=121, open=533, in_progress=101, grading=11,
   blocked=3, tombstone=22** — five of seven drifted. *Positive control:* `grading=11`
   and `tombstone=22` match exactly, so this is drift, not instrument error. No in-tree
   generator: `grep -rln CURRENT-REALITY crates/ --include=*.rs` → 0 files (positive
   control: `decisions.jsonl` → 3 files). → `current-reality-stale-pin-sb8p`.
2. **`boxes/S1.toml` asserts a discharged blocker at four sites.** `:8`, `:249`,
   `:283-285`, and `:339` state `HD-0009..HD-0012` are undecided / `decision=""`. At
   HEAD, `docs/decisions.jsonl:14-17` are answer rows with `decider":"Joshua"` and
   non-empty decisions (385/1187/780/1092 chars). The stated reason S1 cannot be
   approved no longer holds. → `s1-box-asserts-resolved-blocker-cz67`.
3. **`boxes/S1.toml:337` vs `:338`, one line apart** — `refutations = 3` beside a
   verdict justified by "(b) refutations = 0". The `draft` status survives on the other
   stated ground (the functional floor, 4 layers at `exists = "none"`); its cited
   arithmetic does not. → same bead.
4. **`boxes/S2.toml:107-110`** carries a live gap "no per-box diagram; branches exist
   only as prose strings" which `:124-126` refutes 15 lines later
   (`[box.diagram] validate_exit = 0`) and `docs/plan/flow/diagrams/S2.mmd` exists. The
   dead gap was harvested into `maturity0/S2.toml:26` as a live `DEFER_TO_BEAD`
   unknown. Bounded: 1 of 12 boxes, 1 of 12 maturity-0 records. →
   `s2-gap-refuted-by-own-file-6knl`.
5. **`ROUND_LOG.md:13`** (`state=ROUND_5_REQUIRED`) vs **`DESIGN_INDEX.md:3`**
   ("Round 5 integration complete … Round 6 remains required") vs
   **`docs/decisions.jsonl:26`** (rounds 6A+6B ran and returned) vs on-disk
   `docs/planning/round{4,6a,6b,6c,6d}/DEFECTS.toml`. The log has no row for round 4 or
   any 6-series round; positive control `grep -c 'ROUND 5'` → 1. →
   `round-log-four-rounds-stale-atmo`.
6. **`DESIGN_INDEX.md:215-231, :241, :275, :277`** harvest `HD-0001..HD-0017` while the
   ledger holds 18 unique ids through `HD-0018`, breaking the file's own rule at `:211`
   and `:277`; `DEC=17` and `IDS_AFTER=51` should read 18 and 52. The `HD-0018` ask row
   is timestamped `2026-09-04T20:13:24Z`; `DESIGN_INDEX`'s last commit `92acadd` is
   `20:24:22Z` — edited **11 minutes after** the row existed. →
   `design-index-hd-harvest-stale-f02j`.
7. **`docs/plan/flow/CONTRACT.md` carries three surfaces and three superseding
   amendments with nothing marking which is live**: target-state row shape `:10-34`;
   Charter-level BUILD FREEZE `:57-59`; execution authorization `:101-111` ("S1 IS
   AUTHORIZED TO BUILD"); and `:125-151` superseding that in turn while recording
   execution state ("Two S1 builds are suspended, not cancelled"). The gate at
   `:148-150` is now **discharged** by `c47aec8` and still reads as in force. →
   `r1-relocations-no-integrator-1xx9` acceptance 4.
8. **`docs/plan/00-brief.md` carries Charter + current reality + plan index in one
   file**: Charter `:16-80`, current reality `:146` and `:507`, plan index `:543`,
   target-state writing contract `:565-610`, risk/kill criteria `:774`. The
   distinction is explicit in prose — every fact is labelled MEASURED / PROJECTED /
   HISTORICAL (`:299`) and that discipline should be kept — but it is not
   machine-visible, and `R1`/`R13` collide with atlas-arc's rule numbers
   (`00-brief.md:21` requirement R1 vs `CONTRACT.md:137` rule R1). →
   `charter-surface-not-machine-visible-qwyr`.

## Where the depth went

`HD-0014` made R1 binding at `2026-09-03T21:16:19Z`. Churn since, measured per surface:

```
docs/plan/flow/maturity0     1 commit   +512  -0     (c47aec8, landed 21:25Z, +9 min)
docs/plan/flow/boxes         1 commit     +1  -1     (S8.toml only)
docs/plan/[0-9][0-9]-*.md    0 commits    +0  -0
docs/plan/flow/CONTRACT.md   1 commit    +86  -0
docs/planning               14 commits +1253 -156
docs/PLAN.md                 4 commits  +187 -610
```

The R1 remediation **did** land: `maturity0/S1..S9.toml`, 12 tracked files, 512 lines,
12/12 carrying all eight required fields, 157 unknowns each typed into exactly one R3
disposition. So the gate at `CONTRACT.md:148-150` is satisfied — **and the delta did
not move**, because a maturity-0 record is level 0 by construction. Meanwhile the
seven relocations that *would* move it (`S1.toml:28-33, :102-117, :258-269, :276-280,
:293-296, :321-324` → S7/S8) exist only in a bead comment on
`omp-orchestrator-arc-maturity0-intake-i0uh`, which is still `in_progress` with
`updated_at 2026-09-03T21:30:13Z`, and no bead owns the integration
(`relocat` across all 791 rows → 2 hits, both proposing beads).

This repo already recorded the class one level up, at `CONTRACT.md:113-117`: *a
dispatch-only instruction is an unrecorded requirement.*

## Product filed by this pass

`r1-no-checker-d81g` (P0) · `r1-relocations-no-integrator-1xx9` (P0) ·
`r1-exception-missing-so9x` (P1) · `current-reality-stale-pin-sb8p` (P0) ·
`round-log-four-rounds-stale-atmo` (P1) · `design-index-hd-harvest-stale-f02j` (P2) ·
`s1-box-asserts-resolved-blocker-cz67` (P1) · `s2-gap-refuted-by-own-file-6knl` (P2) ·
`charter-surface-not-machine-visible-qwyr` (P1)

Cited as already tracked, not re-filed: `arc-maturity0-intake-i0uh` (delivered,
verified, commented) · `arc-current-reality-2d1q` (commented, plus a correction to my
own count) · `cbp4` (commented: its evidence path `unknowns/DEFECTS.jsonl` does not
exist; the rows are real in `DEFECTS.toml:16` and `:28`) ·
`arc-s4-apply-6c-dispositions-jy4o` · `arc-typed-unknowns-02ai` ·
`coverage-durable-join-8x85` · `denominator-invisible-growth-7t33`.

## Instrument errors made and corrected in this pass

1. A first-pass column-aligned census reported `non-goal = 2` for `00-brief.md`. Two
   independent engines then agreed on **0**. Every published count above was re-run
   with `grep -oE` **and** a second regex engine and they agree; a disagreement would
   be marked.
2. A first-pass churn measurement scoped to `docs/plan/flow/boxes/` concluded the R1
   remediation had produced nothing. It had — in the sibling directory
   `docs/plan/flow/maturity0/`, which the scoped probe could not see. This is the
   wrong-input verdict class this repo filed as `xv30`.
3. `beadrefs=273` at `CONTRACT.md:143` and inside the `HD-0014` body is **not
   reproducible**: on `S1.toml` at HEAD and at blob `e5ab541`,
   `grep -o 'omp-orchestrator-[a-z0-9.]*' | wc -l` → 4 (3 unique);
   `grep -oE '\b[0-9a-z]{4}\b' | wc -l` → 898; the sibling `S1-COVERAGE.md` → 58. No
   instrument returns 273, and no command is recorded beside it, against
   `CONTRACT.md:215-216`.

## NO-CLAIM

This pass advanced no section's maturity and certified no `BUILD_READY` — that
predicate is UNRUN for want of `arc.py`, and per atlas-arc an unavailable tool is
`UNKNOWN`, never a pass. It measured the invariant and filed the defects; it did not
build the checker, integrate any relocation, or edit a single box, section, contract,
or crate.

# R2 — stable IDs before prose growth: the sixteen-kind census

**Rule:** Atlas Arc R2, `~/.claude/skills/atlas-arc/SKILL.md:77-98`. Every normative or schedulable
object must have a durable ID; sixteen kinds are named; "Do not create duplicate IDs for
restatements. References must point to the canonical owner."

**Measured:** 2026-09-05 by `AtlasArcPass2` (skill-loop pass 2 of 10). `BUILDS_RUN=1` — one
single-crate test, named below. No crate edits; `crates/**` is held by panes `%7` and `%8`.

**No oracle for R2 exists on this host.** `atlas-arc` ships `SKILL.md` + `references/` only; there is
no `scripts/arc.py`, so `init`, `lint`, `status`, `certify-plan` cannot run and `BUILD_READY` is
**UNRUN**, never a pass. The sibling `planning-arc` v1.2 `scripts/validate_plan.py` was checked as a
substitute and reports `ids:` counts, so it is a partial oracle for *duplicate* ids but not for
canonical-owner resolution; every figure below is hand-measured and carries its instrument.

## Instrument

**Corpus, chosen explicitly (CENSUS-EXCLUSIONS rule 4 requires picking one).** The *constitution
head* `docs/PLAN.md:1-199` **plus** the thirteen sections `docs/plan/[0-9][0-9]-*.md` — never
PLAN.md's body summed with the sections. Rule 4 as written says pick one, and picking the sections
alone loses a register; see "Correction to a binding rule" below. Consumer population for
"is it referenced by something that consumes it": `docs/**`, `crates/**`, `.github/**`,
`.beads/issues.jsonl`, `SCHEMAS.toml`, `NUMBERS.toml`, `AGENTS.md`, excluding `target/`,
`.rch-tmp/`, `.beads/.br_history/`.

**CENSUS-EXCLUSIONS rules 1-5 APPLIED.** Rule 1 — no `cp-*` id counted (they are control-plane;
`ISSUE_NOT_FOUND` here is expected, not a dangling reference). Rule 2 — `HD-9999` at
`docs/plan/12-journey.md:471` is the F4 planted known-bad and is **excluded**, which is what turns
PLAN.md's 12 ids into 11 live. Rule 3 — `success:[N]` refuse-literals not counted as ids. Rule 4 —
one corpus, as stated above, with the correction recorded. Rule 5 — no hex token treated as a commit
without `git cat-file -e`; both shas cited here were verified that way.

**Positive controls (a zero from a pattern that cannot match is not evidence of absence).**

| control | expectation | result |
|---|---|---|
| `\bGATE-\d\d\d\b` over the corpus + consumers | known present (24 minted in round 6C) | **24 unique, 128 occurrences** — hits |
| `HD-9999` over `docs/plan/12-journey.md` | known planted specimen | **found at `:471`** — hits |
| `DCL-L5-CLAIM-DIES-WITH-WORK` over `docs/contracts/*.md` | known law id | **True** — hits |
| `[[box.measurement]]`/`[[box.gap]]` table count vs `CONTRACT.md:143-144` | five figure families | **all five reproduce exactly** (below) |
| index line-pin resolver, MISS branch present | must be able to report a miss | **24/24 resolve, 0 misses reported** |

## 1. The sixteen-kind table

`LITERAL` = instances of the atlas-arc prefix itself. `ANALOGUE` = this repo's equivalent family,
which is where the content actually lives. `CONSUMER?` = referenced outside its own definition site
and outside the index/round records that minted it.

| # | kind | literal instances | analogue in this repo | canonical owner | consumer? |
|---|---|---|---|---|---|
| 1 | `REQ-*` requirement | **13 ids / 22 occ** | `R1`–`R13` (unnumbered) | **NO_OWNER** — cut at `docs/planning/DESIGN_INDEX.md:275`; all 22 occurrences are `affected_ids` in `docs/planning/round3/DEFECTS.toml` + `round4/DEFECTS.toml`. Source authority `docs/plan/00-brief.md:21-80` carries no `REQ-` string | **no** |
| 2 | `DEC-*` decision | 0 | `HD-0001`..`HD-0018` | `docs/decisions.jsonl` — 26 rows, **18 unique**, **8 duplicate id pairs** | **yes** — `crates/decision-ledger/tests/ledger.rs` (8), `crates/convergence-stamp/src/lib.rs` (7), tracker (63) |
| 3 | `IF-*` interface/protocol | 0 | `DCL-*` (10), `AS-*` (8), `DJ-*` (21) | one `docs/contracts/*.md` file per family (`dispatch_claim_contract.md`, `ack_spine_contract.md`, `dispatch_journey_mapping_contract.md`) | **partial** — 0 of these 39 in `crates/**` |
| 4 | `DATA-*` durable data contract | 0 | `SCHEMAS.toml [artifacts.*]` — **16 declarations** | `SCHEMAS.toml`, readers tabulated at `DESIGN_INDEX.md:48-65` | **yes** — e.g. `artifacts.findings_ledger` → `crates/no-shell-gate/tests/findings_ledger.rs` |
| 5 | `STATE-*` state machine | 0 | `CAN-*` (16), `LIFECYCLE-R1-STATE-AUTHORITY`; plus the verdict enums named at `docs/plan/flow/CONTRACT.md:47-48` (`AckAction`, `PaneState`, `Liveness`, …) | `docs/contracts/cancellation_contract.md`, `lifecycle_contract.md:52`; the enums are Rust types with **no ID** | **partial** |
| 6 | `AUTH-*` authority/trust boundary | 0 | `KOP-*` — 16 ids | `docs/contracts/kernel_only_policy.md` | **weak** — only `.beads/issues.jsonl` |
| 7 | `INV-*` invariant | **13 ids / 48 occ** | `INV-2026` + `INV-L0-*` (9) | split three ways: `INV-2026` → `docs/PLAN.md:40`; `INV-L0-ATOMIC`..`-OBSERVABILITY` → `docs/contracts/s1_l0_install.md`; **`INV-001`..`INV-012` → NO_OWNER** (cut, 22 occ in round3/4 only) | **mixed** — `INV-2026` yes, `INV-00x` no |
| 8 | `CLAIM-*` empirical/formal claim | 0 | `GT-P1`..`P6` (12), `VC-L1`..`L5` (7), `CSL`/`CSR` (2), `docs/plan/HYPOTHESES.jsonl` | `ground_truth_contract.md`, `verification_contract.md`, `claim_strength_contract.md`. A `registries/claims.toml` is named as a **GAP** by `docs/plan/flow/boxes/S2.toml:97` | **partial** |
| 9 | `RISK-*` risk | **6 ids / 10 occ** | unnumbered risk table `docs/plan/01-idea.md:433-438` | **NO_OWNER** — cut at `DESIGN_INDEX.md:275`; all 10 occurrences in round3/4 `DEFECTS.toml` | **no** |
| 10 | `UNK-*` unknown | 0 | `GAP-S1-01`… + `WAVE-<file>#NNN-…` — **269 unique ids** | `docs/plan/flow/unknowns/DISPOSITIONS.toml` (3,557 lines) | **no** — `GAP-S1-` occurs in **zero** files outside `DISPOSITIONS.toml` |
| 11 | `GATE-*` convergence/release gate | **24 ids / 128 occ** | — | `DESIGN_INDEX.md:162-185` (per-id line-pin table) + `:239` (family row) | **yes, in code** — `crates/no-shell-gate/tests/wired_lanes.rs:63-87` (25 occ, `56cdd28`). **Section consumers: 0** |
| 12 | `WS-*` workstream | 0 | `WP-001`..`WP-009` | `docs/PLAN.md:87-95` — the only constitution DAG, and it exists in **no section file** | **weak** — tracker (7) + `docs/decisions.jsonl` (2); `crates/**` 0 |
| 13 | `B-*` bead specification | 0 | `omp-orchestrator-<slug>-<hash>` — **802 rows** | `.beads/issues.jsonl` + `.beads/*.db` | **yes**, heavily |
| 14 | `DEF-*` review defect | **14 distinct strings / 31 definition sites** | — | **NO SINGLE OWNER** — counting only lines matching exactly `id = "DEF-NNN"`: `DEF-001` ×7, `DEF-002` ×5, `DEF-003` ×3, `DEF-004` ×3, `DEF-005` ×3, `DEF-006` ×2, `DEF-007`..`DEF-014` ×1. **6 of 14 strings are defined more than once** | **yes, and ambiguously** — 6 references in `DISPOSITIONS.toml:3481-3541` + 1 in `docs/decisions.jsonl` |
| 15 | `RB-*` Atlas Rigor binding | **0** | none | **ABSENT** — `\bRB-\d` returns zero across `docs/**`, `crates/**`, `.beads/issues.jsonl`. The only occurrence of the string `RB-` is prose inside bead `y0dy` | n/a |
| 16 | `REC-*` closure receipt | **0** | none as an ID | **ABSENT** — `\bREC-\d` zero repo-wide. `receipts` occurs 152×, `crates/receiver-receipt` exists with a 7-id `RR-*` law family, but no receipt is addressable | n/a |

**Roll-up.** `ID_KINDS_DEFINED=16`. Kinds with at least one instance under literal-prefix counting:
**5 of 16** (REQ, INV, RISK, GATE, DEF). Kinds with at least one instance once analogues are
admitted: **14 of 16** — only `RB-*` and `REC-*` are truly absent. Kinds resolving to a canonical
owner: **11 of 16** (`REQ`, `RISK`, `DEF` have instances and no owner; `RB`, `REC` have neither).
The three owner-less kinds are the **worse** state R2 names, because their references cannot be
validated at all.

## 2. Earned vs declared vs index-only — re-derived, not inherited

Instrument: unique-set regex
`\b(REQ|DEC|IF|DATA|STATE|AUTH|INV|CLAIM|RISK|UNK|GATE|WS|B|DEF|RB|REC|HD|WP)-([A-Za-z0-9]+)\b`
per file, then set difference.

| figure | asserted | command | returned |
|---|---|---|---|
| `DESIGN_INDEX.md` canonical ids | 51 (`DESIGN_INDEX.md:275`) | unique-set regex over the file | **51** ✓ |
| `docs/PLAN.md` ids | 12 | same over `docs/PLAN.md` | **12** ✓ — `HD-0001`, `HD-9999`, `INV-2026`, `WP-001`..`WP-009` |
| live constitution ids | 11 | 12 minus `HD-9999` per CENSUS-EXCLUSIONS rule 2 | **11** ✓ |
| index-only | 40 | set difference index − PLAN.md | **40** ✓ — `GATE-001..024` + `HD-0002..0017` |
| PLAN-only | — | set difference PLAN.md − index | **exactly `[HD-9999]`** |
| Round 6C dispositions | ADOPT 24 / DEMOTE 16 / DELETE 0 | `docs/planning/round6c/DEFECTS.toml:24-26` | **24 / 16 / 0** ✓, and `:32` records `APPLIED=none` |
| the 24 ADOPT set | `GATE-001..024` | `round6c/DEFECTS.toml:40-66` `adopt = [...]` | **`GATE-001..024`** ✓ |
| `%7`'s binding | `56cdd28` | `git cat-file -e 56cdd28`; `git show --stat` | **exists**; `wired_lanes.rs` +158 lines, 1 file |

**The question none of that answers — minted vs consumed.** Running round 4 DEF-001's own
`verification_method` verbatim ("re-run for every remaining canonical ID excluding DESIGN_INDEX and
`docs/planning/round*`; report the count with at least one non-index consumer"): **51 of 51 earned,
0 index-only.** Round 4's cut worked. But that criterion only asks "does the string appear
somewhere". Classifying the consumer instead:

| family | n | CODE (`crates/`,`.github/`) | SECTION (`docs/plan/NN-*.md`) | TRACKER | verdict |
|---|---|---|---|---|---|
| `GATE-001..024` | 24 | **24** | **0** | 24 | earned in code, invisible in the constitution |
| `HD-0001..0017` | 17 | 8 | 1 | 17 | earned |
| `WP-001..009` | 9 | **0** | **0** | 9 | minted + scheduled, never implemented or restated |
| `INV-2026` | 1 | 0 | 1 | 1 | earned |

So **of the sixteen kinds, exactly two are referenced by a consumer that executes them**: `GATE-*`
(via `wired_lanes.rs`) and `B-*` (via the tracker and every dispatch packet). `DEC-*`/`HD-*` and
`DATA-*`/`SCHEMAS.toml` are referenced by code that reads them. The remaining twelve are read only by
documents.

## 3. Unreproducible figures

Method: for each figure asserted about a named artifact, run an instrument over that artifact. The
instrument is validated first by the figures that **do** reproduce.

| asserted | artifact | command run | returned | verdict |
|---|---|---|---|---|
| `S1 meas=6` (`CONTRACT.md:143`) | `boxes/S1.toml` | count lines `== "[[box.measurement]]"` | 6 | **REPRODUCES** |
| `S1 gap=10` | `boxes/S1.toml` | count `[[box.gap]]` | 10 | **REPRODUCES** |
| `S1 contracts=1` | `boxes/S1.toml` | count `[box.contracts]` | 1 | **REPRODUCES** |
| `S2..S9 meas=5-8` (`:144`) | 11 non-S1 boxes | min/max of `[[box.measurement]]` | 5–8 | **REPRODUCES exactly** |
| `S2..S9 gap=9-19` | 11 non-S1 boxes | min/max of `[[box.gap]]` | 9–19 | **REPRODUCES exactly** |
| `S2..S9 contracts=0` | 11 non-S1 boxes | count `[box.contracts]` | 0 | **REPRODUCES** |
| `S1 diagram=2` | `boxes/S1.toml` | distinct `.mmd` values in `[box.diagram]` (`S1.toml:341-344`) | 2 (`S1.mmd`, `S1-surfaces.mmd`); every other box 1 | **REPRODUCES under a named instrument** |
| `GROWTH_PER_COMMIT=19.22` (`:69,:157`) | plan coverage | arithmetic recorded at `docs/plan/flow/S1-COVERAGE.md:551` — `(211-38)/9` | 19.22 | **REPRODUCES** — this is the negative control: the corpus *can* show its work |
| **`S1 beadrefs=273`** (`:143`) | `boxes/S1.toml` | file is 449 lines; `omp-orchestrator-` → 4 occ / 4 unique; token `beadref` → **0** | 4 | **DOES NOT REPRODUCE** — 273 exceeds half the file's line count. Third independent confirmation (Pass 1, pane 1, this pass). Tracked: `so9x` |
| **`S2..S9 beadrefs=0-3`** (`:144`) | 11 non-S1 boxes | `omp-orchestrator-` occurrences per box | **1–6 occ / 1–3 unique; no box returns 0** | **DOES NOT REPRODUCE at the low end** — the published range's minimum is unreachable. NEW this pass |
| `DEC=17` / `IDS_AFTER=51` (`DESIGN_INDEX.md:275,277`) | `docs/decisions.jsonl` | unique `HD-*` | **18** (`HD-0018` present) | **STALE** — tracked: `f02j` |
| `HD-0001..HD-0018, 18 rows` (`CENSUS-EXCLUSIONS.md:28`) | `docs/decisions.jsonl` | `wc -l` = 26; unique ids = 18 | 26 rows / 18 ids | **IMPRECISE** — 18 is the id count, not the row count |
| `S1 open_disagreements=0` | `waves/**` | not re-run here | — | tracked: `flow/unknowns/DEFECTS.toml:4` `DEF-001` |

**The mechanism that should have caught all of these exists, runs, and is red.** `NUMBERS.toml` is a
34-row load-bearing-figure registry, **34/34 carrying a runnable `command`**, whose stated rule
(`NUMBERS.toml:19-21`) is "a load-bearing figure gets a row here. `numbers.rs` RE-RUNS the command and
fails when the answer no longer matches. A number without a runnable command is not measured, it is
remembered." It is enforced by `crates/no-shell-gate/tests/numbers.rs` and wired into CI at
`.github/workflows/gate.yml:146`.

Executed (`RCH_CARGO_WRAPPER_BYPASS=1 cargo test --offline -p no-shell-gate --test numbers`, 7.6s):

```
running 8 tests ... 7 passed; 1 failed
no_declared_figure_has_drifted FAILED (crates/no-shell-gate/tests/numbers.rs:192)
3 of 34 load-bearing figures have DRIFTED since they were written:
    workspace_crates_on_disk: recorded "65", command now answers "72"   ($ ls -1 crates | wc -l)
    ipg6_root_symbols:        recorded "622", command now answers "611"
    spawning_crates:          recorded "47", command now answers "51"
```

And the coverage gap: substring test over `NUMBERS.toml` for `beadref`, `273`, `MATURITY`,
`GROWTH_PER_COMMIT`, `DECLARED_AT_HEAD`, `IDS_AFTER`, `open_disagreements` → **False for all seven**.
The registry covers 34 figures and none of the figures that set the freeze. Filed as
`omp-orchestrator-numbers-registry-misses-freeze-figures-o2f7`.

**Instrument error made and corrected here, so no later pass inherits it.** I first reported "0
`.github/**` references to `NUMBERS.toml`" and read that as "not in CI". Wrong input: `gate.yml:146`
runs `cargo test -p no-shell-gate`, which *includes* `tests/numbers.rs` without naming the file. The
gate **is** wired. Separately and still true: the last CI run is `4b398e4` (2026-09-02T03:01:50Z)
against 381 unpushed commits, so CI has never seen this tree.

## 4. Duplicate IDs and non-owner references

| finding | evidence | disposition |
|---|---|---|
| `DEF-001` defined **7 times** as 7 different objects — P0/P1/P1/P1/P2/P2/P3, seven distinct `class` values | `flow/unknowns/DEFECTS.toml:4`, `round3:35`, `round4:61`, `round6a:40`, `round6b:57`, `round6c:97`, `round6d:45` | **NEW** → `def-id-namespace-collision-vjfo` |
| `DEF-002` defined 5 times; **6 live references to a bare `DEF-002`** with 5 candidate owners | `DISPOSITIONS.toml:3481,3493,3505,3517,3529,3541`; intended owner `flow/unknowns/DEFECTS.toml:16` | same bead |
| `docs/decisions.jsonl` carries **8 duplicate id pairs** — `HD-0009`..`HD-0013`, `HD-0014`, `HD-0015`, `HD-0018`; ask-row (`decision=""`) + answer-row (`answers=<id>`), neither with `supersedes` | 26 rows / 18 unique ids | tracked `cbp4` (scope `HD-0009..0014`); **`HD-0015` and `HD-0018` are new instances that postdate the filing** — commented, not re-filed |
| `DESIGN_INDEX.md:241,275,277` asserts `HD-0001`–`HD-0017` / `DEC=17` against an append-only ledger holding 18 — an index asserting a stale count of the ledger it points at | `docs/decisions.jsonl:25-26` | tracked `f02j` |
| 31 cut aliases survive as **54 `affected_ids` occurrences** with no owner: `REQ-001..013`, `INV-001..012`, `RISK-001..006` | `round3/DEFECTS.toml`, `round4/DEFECTS.toml` only; `DESIGN_INDEX.md:275` records the cut of exactly 31 | **NEW** → `cut-aliases-left-dangling-references-m3v6` |
| `GATE-001..024`'s only definition site is `DESIGN_INDEX.md:162-185`, a file describing itself (`:9-12`) as "a deterministic alias … does not add product scope" — while `crates/no-shell-gate/tests/wired_lanes.rs` depends on those ids and the aliased sections carry the string 0 times | `GATE-0xx` in `docs/PLAN.md` → 0; in all 13 sections → 0 | tracked `jy4o` (24 ADOPT, `APPLIED=none`) — measured and commented |
| `docs/contracts/**` carries **238 distinct law ids in 27 families**; **0 of 238** appear in `DESIGN_INDEX.md`, `docs/PLAN.md`, or the 13 sections, while `DESIGN_INDEX.md:69-70` declares all 32 contract files normative. **228 of 238 have no `crates/**` consumer**, and — scope stated precisely so the number is not over-read — **81 of 238 have SOME consumer outside `docs/contracts/`** (union of `crates/**` 10, `.beads/issues.jsonl` 37, `docs/plan/flow/**` 41), leaving **157 with no consumer of any kind** | 27 families each resolving to one owning file (`LAW-*` resolves at the two-segment prefix: `LAW-JQ-*`, `LAW-L0-*`..`LAW-L5-*`) | **NEW** → `contract-law-ids-unregistered-a6b7` |
| 269 typed unknown ids in `DISPOSITIONS.toml` have **zero** consumers outside that file | `GAP-S1-` over the consumer population → 0 files | R3's subject (pass 3); recorded, not filed here |

`DUPLICATE_IDS = 14` distinct id strings minted more than once for different objects: **6 `DEF-*`
strings** (`DEF-001` ×7, `DEF-002` ×5, `DEF-003` ×3, `DEF-004` ×3, `DEF-005` ×3, `DEF-006` ×2 —
31 definition sites under 14 strings) and **8 `HD-*` ask/answer pairs**.
`RESTATEMENTS_POINTING_AT_NON_OWNER = 61`: 54 dangling `affected_ids` + 6 bare `DEF-002` references
+ 1 bare `DEF-004` in `docs/decisions.jsonl`.

**Roll-up figures, with their composition so each is reproducible.**
`IDS_TOTAL = 1422` distinct ids across every kind and analogue: `GATE` 24 + `HD` 18 + `WP` 9 +
`INV-2026` 1 + `REQ` 13 + `INV-001..012` 12 + `RISK` 6 + `DEF` 14 + contract laws 238 +
typed unknowns 269 + `SCHEMAS.toml` artifacts 16 + bead ids 802.
`IDS_EARNED = 960` (same order: 24 + 18 + 9 + 1 + 0 + 0 + 0 + 9 + 81 + 0 + 16 + 802) — an id is
earned when it is referenced outside its own definition site and outside the index/round records
that minted it. `IDS_UNEARNED = 462`, of which **269 are the typed unknowns and 157 are contract
laws** — so 92% of the unearned population is two surfaces, not scattered rot.
`IDS_INDEX_ONLY = 0` under round 4 `DEF-001`'s own `verification_method` re-run over the current
51-id register. That is not a clean bill: **40 of the 51 are absent from the constitution**
(`docs/PLAN.md`), which is `y0dy`'s finding and reproduces exactly. "Index-only" and
"constitution-absent" are different questions and only the first is zero.

## Correction to a binding rule

`CENSUS-EXCLUSIONS.md:14` (rule 4) is binding on this census, and two of its claims do not
reproduce. `docs/PLAN.md` is **not purely assembled**: 125 of its 6,259 nonblank lines appear in no
section, 103 of them below `:200`, and `PLAN.md:6-8` says so — "this header is the constitution's
review metadata and companion boundary." The consequence is not cosmetic: `WP-001`..`WP-009` and the
only constitution DAG live at `PLAN.md:87-95` and in **zero** section files, so obeying rule 4 by
picking the sections reports the `WS-*` kind as ABSENT. My own table read `WS = ABSENT` before this
was caught. Also, rule 4's `8,209 vs 10,019` pair compares `docs/PLAN.md` against a **19-file** glob
that includes six non-section documents; the like-for-like pair is **8,209 vs 7,969**
(`cat docs/plan/[0-9][0-9]-*.md | wc -l`). Filed as `census-rule4-hides-wp-register-1px7`; rule 4
below is amended in place.

## Handoff, out of R2 scope

`workspace_crates_on_disk` drifted `65 → 72`. Independently, `git log --diff-filter=A` on each crate
directory dates **seven crates to after the BUILD FREEZE** of `CONTRACT.md:57-59` (Joshua,
2026-09-03 05:3xZ, "No new crate … until every box has `agreement.status = "converged"`"):
`lifecycle-event` 09-03T20:23Z, `lifecycle-monitor` 20:47Z, `installer` 21:07Z, `s1-coverage`
21:08Z, `omp-types` 09-04T16:50Z, `bead-availability` 09-04T19:43Z, `ntm-fleet-monitor`
09-05T02:49Z. The freeze's exit condition is unmet: `agreement.status` over the 12 boxes is `draft`
×9 and **absent entirely** on `S6a`, `S6b`, `S6c`. Not filed by this pass — it is a freeze question,
not an R2 question. Pane 1 owns the disposition.

## NO-CLAIM

This census does not claim any ID's *content* is correct, does not claim the 238 contract laws are
consistent, and does not certify `BUILD_READY` — no R2 checker exists on this host, so the verdict is
hand-measurement against rule text. It does not claim the 31 covered-and-undrifted `NUMBERS.toml`
figures are right, only that their commands re-run and match. It does not claim CI state: the tree is
381 commits ahead of the last CI run.

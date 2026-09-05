# R3 — unknowns as typed scheduling objects (Atlas Arc pass 3)

**Rule:** `~/.claude/skills/atlas-arc/SKILL.md:100-111`. Every unknown gets **exactly one** of
`BLOCKS_PLAN BLOCKS_BUILD ASSUME_REVERSIBLY DEFER_TO_BEAD HUMAN_DECISION OUT_OF_SCOPE`.
`SKILL.md:111`: *"“Open,” “TBD,” “investigate,” and “decide later” are not valid dispositions by themselves."*

**Measured** 2026-09-05 by `AtlasArcPass3` (non-integrator, read-only toward `crates/**`). `BUILDS_RUN=0`.
Binding instrument rules: `docs/planning/CENSUS-EXCLUSIONS.md` 1-5. No figure below is inherited; every one names its command.

## 0. Instrument controls

| control | command | result |
|---|---|---|
| POSITIVE | `grep -c '^\[\[row\]\]' docs/plan/flow/unknowns/DISPOSITIONS.toml` vs the file's own `# rows=269` (line 2) vs `CENSUS.json:14` | **269 = 269 = 269**, three independent sources; 269 unique `id`, 0 duplicates |
| POSITIVE | `grep -c '\[\[box.gap\]\]' docs/plan/flow/boxes/*.toml` vs `DISPOSITIONS.toml` rows with `surface="box.gap"` | **177 = 177**, and equal per box (10/9/12/13/16/15/16/16/16/19/19/16) |
| POSITIVE | TOML-parsed empty wave resolutions vs `CENSUS.json:6` `wave_resolution_empty_toml_parser` | **84 = 84**; the 84 map 1:1 onto the register's 84 wave rows, symmetric difference **0** |
| NEGATIVE | same instrument, fabricated tokens `DEFER_TO_HUMAN`, `BLOCKS_SHIP`, `ASSUME_IRREVERSIBLY` over `DISPOSITIONS.toml` | **0, 0, 0** — the token search does not invent members |
| NEGATIVE | `br show omp-orchestrator-zzzz-does-not-exist-9999` | `ISSUE_NOT_FOUND` — bead-existence probe distinguishes real from fabricated (positive twin: `br show kxe.8` → `omp-orchestrator-kxe.8 open`) |
| NEGATIVE | regex form `resolution = ""` instead of a TOML parse | **72, not 84** — the wrong instrument returns a wrong answer and I can name the 12 rows it drops (all `wave-1/SilverWolf-*.toml`, single-quoted `''`). This reproduces BOTH published figures and their causes. |

## 1. Population — one total, overlap resolved

`UNK-*` ids: **0** across `docs/**`, `crates/**`, `.beads/issues.jsonl`. `docs/plan/unknowns/` does not exist (the real path is `docs/plan/flow/unknowns/`).

| surface | objects | unknowns | dispositioned | undispositioned |
|---|---|---|---|---|
| `docs/plan/flow/boxes/S*.toml` `[[box.gap]]` | 177 | 177 | 177 | 0 |
| `docs/plan/flow/waves/**` `[[disagreement]]` | 160 | 104 (84 empty + 20 open-worded) | 84 | 20 |
| `docs/plan/flow/boxes/S1.toml` `[[box.hook]]` UNKNOWN | 1 | 1 | 1 | 0 |
| `docs/decisions.jsonl` | 29 rows / 19 ids | 8 (7 dual-row/claim + HD-0019) | 7 | 1 |
| `docs/plan/00-brief.md:808-834` Q1..Q13 + K1..K5 | 18 | 14 | 0 | 14 |
| **TOTAL** | | **304** | **269** | **35** |

**Overlap explicitly handled — the `269` is not additive with `157`.** `docs/plan/flow/maturity0/S*.toml`
carries 157 typed unknowns. They are **not a sixth surface**: they are a SECOND TYPING of the same
`[[box.gap]]` objects already counted in row 1. Proof, not assertion: per-box counts are identical
for S1..S4 (10/9/12/13 in `boxes/`, in `DISPOSITIONS.toml`, and in `maturity0/`), the items appear in
the same order, and the text corresponds 1:1 (`boxes/S1.toml:289` “three S1 names…” = `DISPOSITIONS`
`GAP-S1-04` = `maturity0/S1.toml:23`). Adding 157 to 269 would be the `CENSUS-EXCLUSIONS.md` rule-4
double-count in a new place. 20 box gaps have no `maturity0` counterpart (177−157).

Also excluded, with reason: 56 wave disagreements carry a real resolution (`resolved-at-7f9195a` ×29,
`accepted-and-landed`, `refuted(...)`) and are not unknowns; `wave-1/SilverWolf` and friends contribute
no separate objects beyond their disagreement rows.

## 2. Disposition validity

### 2.1 The primary register — `docs/plan/flow/unknowns/DISPOSITIONS.toml`, 269 rows

| disposition | rows | R3-valid alone? |
|---|---|---|
| `OUT_OF_SCOPE` | 182 | value legal; **0 of 182 cite a charter** (`charter` token in file: 1, none in a rationale) |
| `ASSUME_REVERSIBLY` | 45 | **41 carry no falsifier** → §3 |
| `BLOCKS_PLAN` | 28 | yes |
| `DEFER_TO_BEAD` | 12 | **0 name a bead** → §5 |
| `HUMAN_DECISION` | 1 | names no gate → §4 |
| `BLOCKS_BUILD` | 1 | yes |

**`MULTI_DISPOSITION = 150`.** 150 rows carry a second, out-of-taxonomy key `seventh = "UNBUILT"`
*alongside* `disposition = "OUT_OF_SCOPE"`. Cross-tab over all 269: (OUT_OF_SCOPE, UNBUILT) 150,
(ASSUME_REVERSIBLY, –) 45, (OUT_OF_SCOPE, –) 32, (BLOCKS_PLAN, –) 28, (DEFER_TO_BEAD, –) 12,
(HUMAN_DECISION, –) 1, (BLOCKS_BUILD, –) 1. `CENSUS.json:16-24` reports the same population as
`OUT_OF_SCOPE: 32` + `UNBUILT: 150`, so the JSON and the TOML disagree about what the 150 are.
R3 says *exactly one*; these have two and the second is not in the taxonomy. Tracked: `omp-orchestrator-1qzt`
(commented, not re-filed) — with the new measurement that the seventh class is **already deployed,
unratified, laundered under a legal label**.

### 2.2 The conflicting second register — `docs/plan/flow/maturity0/S*.toml`, 157 rows

| disposition | rows |
|---|---|
| `DEFER_TO_BEAD` | 76 |
| `BLOCKS_BUILD` | 38 |
| `BLOCKS_PLAN` | 34 |
| `HUMAN_DECISION` | 6 |
| `OUT_OF_SCOPE` | 2 |
| `ASSUME_REVERSIBLY` | 1 |

**Index-aligned on the four boxes where both registers hold identical counts (n=44): AGREE 2, DISAGREE 42.**
Only `GAP-S3-12` and `GAP-S4-13` agree. Across all 149 fuzzy-matched rows: AGREE 2, DISAGREE 147,
28 with no counterpart. **33 rows are `BLOCKS_PLAN` in one register and `OUT_OF_SCOPE` in the other** —
the two ends of R3's severity range, asserted about the same object. `git log -1` dates the two files
`ec2f73b 15:26:39` and `c47aec8 15:28:11` on 2026-09-03: **92 seconds apart, neither citing the other**.
→ `omp-orchestrator-r3-two-registers-conflict-0gyc` (P0).

S1 head-to-head, all ten rows, both registers:

| id | `DISPOSITIONS.toml` | `maturity0/S1.toml` |
|---|---|---|
| `GAP-S1-01` | `OUT_OF_SCOPE` | `BLOCKS_BUILD` |
| `GAP-S1-02` | `ASSUME_REVERSIBLY` | `HUMAN_DECISION` |
| `GAP-S1-03` | `ASSUME_REVERSIBLY` | `HUMAN_DECISION` |
| `GAP-S1-04` | `ASSUME_REVERSIBLY` | `BLOCKS_PLAN` |
| `GAP-S1-05` | `OUT_OF_SCOPE` | `DEFER_TO_BEAD` |
| `GAP-S1-06` | `OUT_OF_SCOPE` | `DEFER_TO_BEAD` |
| `GAP-S1-07` | `DEFER_TO_BEAD` | `HUMAN_DECISION` |
| `GAP-S1-08` | `OUT_OF_SCOPE` | `BLOCKS_BUILD` |
| `GAP-S1-09` | `OUT_OF_SCOPE` | `DEFER_TO_BEAD` |
| `GAP-S1-10` | `OUT_OF_SCOPE` | `BLOCKS_PLAN` |

Ten objects, **ten disagreements**.

### 2.3 `INVALID` — verbatim quotes of every rejected phrase

**Wave surface, 20 rows, non-empty resolution that is not a disposition** (none appear in any register):

- `wave-1/BlueLantern-S1.toml#2` [major] — "accepted AND STILL OPEN, deliberately not half-answered. Owner re-measured: the eight [[box.hook]] rows at S1.toml:75-106 still carry surface/hook/status and none of the twelve certified fields (id, 
- `wave-2/WildMountain-S1.toml#12` [blocker] — "remaining: the diagram names class fairness, but the selector has no class starvation state or fairness guarantee; bead availability represents Grading but does not select it, and verify-dispatch ver
- `wave-2/WildStone-S1.toml#12` [major] — "remaining; diagram still misses per-tool bv/fh/rch/jsm/frankenmermaid/launchd/OMP-hook result branches; evidence=S1.mmd:19-28"
- `wave-2/WildStone-S1.toml#16` [major] — "remaining; installer still has zero Cx/Scope and the map names replacement rather than proof; evidence=S1-surfaces.mmd:11"
- `wave-2/WildStone-S1.toml#19` [major] — "remaining; diagram names Cx/process-group policy but no region-owned runtime wrapper is proven; evidence=S1-surfaces.mmd:19"
- `wave-2/WildStone-S1.toml#21` [blocker] — "remaining; command-v new-or-prior identity remains PROJECTED and no implementation edge is shown; evidence=S1-surfaces.mmd:2"
- `wave-2/WildStone-S1.toml#23` [major] — "remaining; diagram has post-spawn branch but no runtime spawn/registration/pack receipt; evidence=S1.mmd:58-63"
- `wave-2/WildStone-S1.toml#24` [major] — "remaining; installer zero-Cx and L4 region-owned spawn remain unresolved; evidence=S1-surfaces.mmd:11,19"
- `wave-2/WildStone-bead-lifecycle.toml#1` [blocker] — "remaining; selection feedback is cyclic and not yet unrolled"
- `wave-2/WildStone-bead-lifecycle.toml#2` [major] — "remaining; GRADE is overloaded across selection and verification"
- `wave-2/WildStone-bead-lifecycle.toml#3` [major] — "remaining; ReceiverDecision ownership and projection are unspecified"
- `wave-2/WildStone-bead-lifecycle.toml#4` [major] — "remaining; ACCEPT must be tied to DispatchVerdict::Delivered and work-start evidence"
- `wave-2/WildStone-bead-lifecycle.toml#5` [major] — "remaining; DENY lacks explicit br update, new research bead, and blocking-edge receipt"
- `wave-2/WildStone-bead-lifecycle.toml#6` [major] — "remaining; research class/label mapping and materializer contract are unspecified"
- `wave-2/WildStone-bead-lifecycle.toml#7` [major] — "remaining; deadman cannot distinguish intentional denial from no-packet starvation"
- `wave-2/WildStone-bead-lifecycle.toml#8` [major] — "remaining; LifecycleEvent does not yet carry the complete ACK/DENY receipt join"
- `wave-2/pane1-S1.toml#2` [blocker] — "escalated: row stays open and widens beyond S1 — the ten-site raw ntm spawn census is a cross-cutting Cx finding, not an S1 diagram defect"
- `wave-2/pane1-S1.toml#5` [major] — "accepted: row stays open; the resolving artifact is a Probe verdict enum carrying OK|ABSENT_FAMILY|ABSENT_SPECIFIC|UNPROBEABLE|STALE|PAUSED|UNMEASURED, not additional diagram nodes"
- `wave-2/pane1-S1.toml#7` [major] — "accepted: row stays open; resolving artifact is a SCHEMAS.toml intake-requirements row with the persona derivation rule, referenced by S1.toml:19"
- `wave-2/pane1-S1.toml#8` [blocker] — "escalated: canonical type is controller-tick::OutcomeClass (reuse mandated, do-not-fork), but that crate is pending-extraction from control-plane — S1's refusal taxonomy is blocked on the extraction,

`remaining`, `escalated:`, `row stays open`, `accepted AND STILL OPEN` are the `SKILL.md:111` class:
a status, not a disposition. 4 of the 20 are severity `blocker`.

**`docs/plan/00-brief.md`, 14 rows whose recorded disposition is literally the word R3 rejects:**

- `:810` Q1 — “Who pays for this, and what is their current workaround? | **OPEN** — no buyer named anywhere in thirteen sections | Josh”
- `:811` Q2 **OPEN** · `:812` Q3 **OPEN** · `:813` Q4 **OPEN** · `:814` Q5 **OPEN** · `:815` Q6 **OPEN** · `:816` Q7 **OPEN** · `:817` Q8 **OPEN** · `:819` Q11 **OPEN** · `:820` Q12 **OPEN** · `:821` Q13 **OPEN**
- `:822` Q10 — “**What kills this?** | **PARTIAL** — Josh owns the decision … no economic criterion or decision receipt is recorded”
- `:831` K2 — “**OPEN/UNVERIFIED — owner: Josh. … Instrumentation and baseline are not yet built.**”
- `:834` K5 — “**OPEN/UNVERIFIED — owner: orchestrator. …**”

`:849` states the count itself: *“This section registers thirteen questions (eleven OPEN; Q9 ANSWER MOVED; Q10 PARTIAL)”*.
`:789` states the rule this satisfies and R3 rejects: *“must be either answered or **registered here as an
open question with an owner**”*. Registering with an owner is not a disposition.

**And the sections type nothing at all.** For each of the six tokens,
`grep -oh "$t" docs/plan/[0-9][0-9]-*.md | wc -l` → **0 0 0 0 0 0**; same over `docs/PLAN.md:1-199` → 0.
Positive control on the identical corpus: `grep -oh 'S1' … | wc -l` → 40. → `omp-orchestrator-r3-plan-sections-type-no-unknowns-yb4q`.

**`NONE` = 1:** `HD-0019` (2026-09-05T03:06:44, the only unanswered ledger id, `binds_stages=["S9"]`,
blocking `omp-orchestrator-heldout-acceptance-evidence-wotw` which `br show` confirms open) carries no
disposition anywhere. `HD-0015..HD-0019` all postdate the census and are in no register.

## 3. `ASSUME_REVERSIBLY` without a falsifier — 42 of 46

R3 requires *“a documented reversible assumption **and falsifier**”*. An assumption with no falsifier
can never be retired: nothing can show it wrong. It is a permanent silent commitment wearing a temporary label.

**Falsifier present — 4 of 45, verbatim:**

- `WAVE-wave-1/AmberGate-S5a-S5b-S7-S8.toml#001-S5a-all` — reviewer asserting-correct; owner_says stands; falsifier=re-run measurements
- `GAP-S1-02` — install as ompo; falsifier=command -v installer points at /usr/sbin after install
- `GAP-S1-03` — accept diagram 0ea808a skip-spawn branch; falsifier=S1.mmd lacks persona A skip
- `GAP-S4-05` — accept internal atom-correctness until an out-of-tree arbiter is named; falsifier=named oracle row

Only `GAP-S1-02` is executable as written (`command -v installer`). The other three name a class of evidence, not a check.

**Falsifier absent — 41 of 45.** Full list with verbatim reasons:

- `WAVE-wave-1/AmberGate-S5a-S5b-S7-S8.toml#002-S5a-kernel_output` — additive cite of existing DispatchIntent; not a Joshua question
- `WAVE-wave-1/AmberGate-S5a-S5b-S7-S8.toml#003-S5b-all` — reviewer asserting-correct
- `WAVE-wave-1/AmberGate-S5a-S5b-S7-S8.toml#006-S7-branches` — reading-hazard note; seven GATE banners vs eight gates; owner amends prose
- `WAVE-wave-1/AmberGate-S5a-S5b-S7-S8.toml#007-S8-all` — asserting-correct plus additive headline
- `WAVE-wave-1/AmberGate-S5a-S5b-S7-S8.toml#008-S8-crate_status` — installed-vs-in-repo nuance; crate_status stays exists-no-caller
- `WAVE-wave-1/AmberGate-S6a-S6b-S6c.toml#009-S6a-all` — asserting-correct
- `WAVE-wave-1/AmberGate-S6a-S6b-S6c.toml#010-S6a-measurement` — stale: S6a now has 16 gap rows so the four sections exist
- `WAVE-wave-1/AmberGate-S6a-S6b-S6c.toml#013-S6c-all` — asserting-correct
- `WAVE-wave-1/AmberGate-S9.toml#017-S9-crate_status` — opposing point on the same change; keep hook-wired not plain wired
- `WAVE-wave-1/AmberGate-S9.toml#018-S9-measurement` — stale: S9 now has 16 gap rows
- `WAVE-wave-1/AmberGate-S9.toml#019-S9-branches` — richer arms are a map amend, not a human halt
- `WAVE-wave-1/BlueLantern-S2.toml#021-S2-hook` — hook schema is HOOK-SCHEMA-CORE 10 fields (vnp2); do not ask Joshua
- `WAVE-wave-1/BlueLantern-S3.toml#024-S3-gap_class_coverage` — 16-class walk is CONTRACT.md; owner amends the gap list
- `WAVE-wave-1/BlueLantern-S3.toml#025-S3-hook_schema` — hook schema 10-field core
- `WAVE-wave-1/BlueLantern-S4.toml#029-S4-hook_schema` — hook schema 10-field core
- `WAVE-wave-1/BlueLantern-S6a.toml#031-S6a-branches` — verbatim enum arms is CONTRACT; owner splits the grouped string
- `WAVE-wave-1/BlueLantern-S6a.toml#032-S6a-hook` — missing hook row becomes an explicit absence row, not a Joshua call
- `WAVE-wave-1/BlueLantern-S6b.toml#036-S6b-hook` — explicit [[box.hook]] absence row
- `WAVE-wave-1/BlueLantern-S6c.toml#037-S6c-branches` — MISSING Grade is a gap not a branch; owner moves it
- `WAVE-wave-1/BlueLantern-S6c.toml#039-S6c-hook` — explicit hook absence
- `WAVE-wave-1/BlueLantern-S7.toml#040-S7-gap_class_schema` — class vocabulary is CONTRACT.md; owner renames
- `WAVE-wave-1/BlueLantern-S7.toml#041-S7-hook_schema` — reachability is not certification; absence row
- `WAVE-wave-1/BlueLantern-S8.toml#045-S8-hook` — auto-install HUMAN is GAP-S8-18; this row is only the absence schema
- `WAVE-wave-1/BlueLantern-S9.toml#046-S9-gap_class_schema` — class vocabulary
- `WAVE-wave-1/BlueLantern-S9.toml#048-S9-hook` — human-halt IS the trigger; no hook
- `WAVE-wave-1/QuietRidge-S2.toml#049-S2-gap_inventory` — 16-class walk
- `WAVE-wave-1/QuietRidge-S2.toml#050-S2-agreement.approval` — blank approval writes MISSING; not a Joshua interrupt
- `WAVE-wave-1/QuietRidge-S3.toml#051-S3-gap_inventory` — 16-class walk
- `WAVE-wave-1/QuietRidge-S4.toml#053-S4-gap_inventory` — 16-class walk
- `WAVE-wave-1/QuietRidge-S5a.toml#056-S5a-branches` — strip prose arrows from branch strings
- `WAVE-wave-1/QuietRidge-S5b.toml#057-S5b-gap_inventory` — 16-class walk
- `WAVE-wave-1/QuietRidge-S5b.toml#058-S5b-kernel_output` — additive type cite
- `WAVE-wave-1/QuietRidge-S6a.toml#059-S6a-branches` — split collapsed ReceiptReason
- `WAVE-wave-1/QuietRidge-S6a.toml#060-S6a-agreement.approval` — approval blank -> MISSING
- `WAVE-wave-1/QuietRidge-S6b.toml#062-S6b-agreement.approval` — approval blank -> MISSING
- `WAVE-wave-1/SilverWolf-S1.toml#067-S1-wave1_sections` — stale: S1 now has measurement+10 gaps
- `WAVE-wave-2/pane4-jt1i-citations.toml#081-S1-cite.mirror-truncated-ellipsis` — truncated mirror ellipsis already refused by CONTRACT.md
- `WAVE-wave-2/pane4-jt1i-citations.toml#083-S1-figure.unlabelled-in-owner-files` — unlabelled figure in owner files; not a halt
- `WAVE-wave-2/pane4-qibn-hook-count.toml#084-S1-box.hook.certified_false_vs_unattempted` — certified=false vs UNATTEMPTED already amended in S1.toml hook rows
- `GAP-S1-04` — spine.mmd is canonical (renderer source); boxes/S1.toml and 12-journey cite it
- `GAP-S3-06` — HD-0002 already: Josh is the buyer and the external loop; do not re-ask

Plus `maturity0/S8.toml`'s single `ASSUME_REVERSIBLY`: the `unknowns[]` schema has exactly two keys
(`disposition`, `item`), so **no falsifier field exists in any of the twelve files** — 12/12 structurally
incapable. Union: **42 of 46**. Whole-file `falsifier` token count in `DISPOSITIONS.toml`: **5**
(4 in `ASSUME_REVERSIBLY` reasons, 1 elsewhere). The repo can do this correctly — exactly once, at
`wave-2/pane4-41li-cross.toml`: *“accepted: null sibling; **re-open when 6gh6 or d9rv lands a file under
docs/contracts/**”* — a falsifier that is a runnable `ls`. → `omp-orchestrator-r3-assume-reversibly-no-falsifier-tlae` (P0).

## 4. `HUMAN_DECISION` — 6 objects, 0 name a gate, 0 actuated

R3 says `HUMAN_DECISION` *“requires an explicit human choice before a named gate”* — and **stops there.**
The rule specifies no completion path, and the repo shows what that costs.

| unknown | register | names a gate? | ledger row? | consequence executed? |
|---|---|---|---|---|
| `GAP-S8-18` / `maturity0/S8.toml#14` — Joshua must authorize automatic certified-hook installation | both | **NO** (“the hook”, no id) | none | **NO** |
| `maturity0/S1.toml:21` — installer name must avoid the `/usr/sbin/installer` PATH collision | maturity0 | **NO** | none (`ompo` → 3 hits, all inside HD-0016's unrelated question text) | **NO** |
| `maturity0/S1.toml:22` — Persona A not-live skip-swarm branch needs accept/reject | maturity0 | **NO** | none (`grep -ci persona` → 0) | **NO** |
| `maturity0/S1.toml:26` — SessionStart is 0, launchd paused, slash start absent until HD-0009 | maturity0 | **NO** | HD-0009 **is** answered | **NO** — `omp-orchestrator --help` still exposes no `doctor`/`init`/`portal`/`start` verb |
| `maturity0/S3.toml#6` — no out-of-our-control oracle establishes the plan is good | maturity0 | **NO** | HD-0002 arguably answers it | **NO** — maturity0 still types it `HUMAN_DECISION`, so the answer never propagated |
| `maturity0/S4.toml#5` — no external arbiter for the nine-part atom | maturity0 | **NO** | none | **NO** |

`grep -c 'GATE-' docs/decisions.jsonl` → **0**. Positive control `grep -c binds_stages` → **29**.
The ledger's only binding vocabulary is STAGES; R3's “named gate” has no carrier in this repo.
`would_have_asked_joshua = true` appears on **1 of 269** rows; `CENSUS.json:34` records
`MOVED_OFF_HUMAN_DECISION: 20` — the census optimised for *minimising* this disposition, which is why
1 survived in a corpus `maturity0` types as 6.

**`HUMAN_DECISIONS_UNACTUATED = 6 of 6.**

Tracked, cited not re-filed: `omp-orchestrator-decision-ledger-needs-actuator-lef1` (P0, open).
Re-measured for it: **`git rev-list --count origin/main..HEAD` → 394** unpushed commits (183 at HD-0008's
decision on 2026-09-02; 211 more since). **What this census adds that `lef1` does not:** the unactuated
population is larger than the ledger — **0 of these 6 have a ledger row at all**, so an actuator installed
on `docs/decisions.jsonl` would find nothing to actuate for any of them. → `omp-orchestrator-r3-human-decision-names-no-gate-w73c`.

## 5. `DEFER_TO_BEAD` without a bead — 83 of 88

**`DISPOSITIONS.toml`: 12 rows, 0 name a bead. 11 of 12 name a PERSON.** Verbatim:

- `WAVE-wave-1/BlueLantern-S2.toml#022-S2-agreement` — owner recounts open_disagreements after this wave
- `WAVE-wave-1/BlueLantern-S3.toml#026-S3-agreement` — owner recounts agreement
- `WAVE-wave-1/BlueLantern-S4.toml#027-S4-kernel_input_citation` — owner supplies path:line citation
- `WAVE-wave-1/BlueLantern-S4.toml#030-S4-agreement` — owner recounts agreement
- `WAVE-wave-1/BlueLantern-S6a.toml#033-S6a-agreement` — owner recounts agreement
- `WAVE-wave-1/BlueLantern-S7.toml#042-S7-agreement` — owner recounts agreement
- `WAVE-wave-1/QuietRidge-S3.toml#052-S3-sota_cite` — owner adds :L<n> to sota_cite
- `WAVE-wave-1/QuietRidge-S4.toml#054-S4-sota_cite` — owner adds :L<n>
- `WAVE-wave-1/SilverWolf-S1.toml#068-S1-sota_cite` — owner adds line cite or drops the gold-standard claim
- `WAVE-wave-2/pane4-jt1i-citations.toml#079-S1-cite.beads-rust-cli-paths-unprefixed` — owner prefixes mirror:beads_rust/ on doctor.rs paths
- `WAVE-wave-2/pane4-jt1i-citations.toml#080-S1-cite.bare-src-main-without-crate` — owner prefixes crate on bare src/main.rs
- `GAP-S1-07` — HD-0009 is answered (frankentui+json); remaining work is the spawn trigger bead

**`maturity0/S*.toml`: 76 rows, 5 carry a bead-shaped token**, resolving to 3 distinct beads.
Bead existence checked by command — the failure is *naming*, not dangling references:

| token | `br show` | status |
|---|---|---|
| `jplf.7.3` | `omp-orchestrator-jplf.7.3` | open |
| `kxe.8` | `omp-orchestrator-kxe.8` | open |
| `vcd7.1` | `omp-orchestrator-plan-11-vcd7.1` | open |
| *(negative control)* `omp-orchestrator-zzzz-does-not-exist-9999` | — | `ISSUE_NOT_FOUND` |

3/3 exist and are open. The other 71 name nothing: e.g. `maturity0/S5a.toml#2` *“No selector fuzz target
covers malformed br/bv rows, claimability, or epic filtering.”*, `S5b#4` *“No S5b SLO covers authorization
latency or packet-preflight refusal rate.”* Real work, no bead, so no `bv` ready-frontier can ever surface it.
**`DEFERRED_WITHOUT_BEAD = 83`.** → `omp-orchestrator-r3-defer-to-bead-names-no-bead-9bt4`.

## 6. The instrument defect that produced the gap

The register's unknown-detector is provably `resolution == ""` and nothing else: its 84 wave rows match
the 84 empty-resolution disagreements with **symmetric difference 0** on the (file, field) key. So a row
reading `"remaining; selection feedback is cyclic and not yet unrolled"` is non-empty → read as RESOLVED →
never dispositioned. All five files holding the 20 skipped rows were last written ≤ 2026-09-03 12:32,
~3 h before `ec2f73b` 15:26 — **available and skipped, not stale**.

And `CENSUS.json:7` explains its own 84-vs-72 discrepancy as `"72 (undercounts missing-key rows)"`.
**Zero rows are missing the key** — all 160 carry `resolution`. The real cause is **12 rows writing
`resolution = ''` with single quotes**, every one in `wave-1/SilverWolf-*.toml` (S1 ×2, S2, S3, S4, S5a,
S5b ×2, S7 ×2, S8 ×2). Right number, wrong cause: the next regex census reproduces 72 and re-derives the
wrong reason. → `omp-orchestrator-r3-unknown-detector-is-empty-string-3zhb`.

## 7. AN R3 ORACLE DOES RUN HERE — and it reports a clean 0

**Correction to a standing loop caveat, and the sharpest finding of this pass.** The loop header records
that `atlas-arc` ships no `scripts/arc.py`, so no checker runs. For R3 that is wrong.
`~/.claude/skills/planning-arc/scripts/validate_plan.py:47` carries the exact six-token tuple and `:610`
emits `open questions without a typed disposition`.

| leg | command | result |
|---|---|---|
| instrument validated FIRST | `python3 validate_plan.py --selftest` | exit 0 — `SELFTEST PASS: planted defects (undefined ref, duplicate id, cycle, TBD, **undispositioned OQ**) detected` |
| as-is, 14 files | `validate_plan.py <each of docs/plan/[0-9][0-9]-*.md, docs/PLAN.md> --json` | `undispositioned_open_questions = []` on **14 of 14**; `id_counts = {}` on **14 of 14** |
| rename only | `sed -E 's/^\| Q([0-9]+) \|/| OQ-\1 |/' docs/plan/00-brief.md` (13 rows, scratch copy, no other edit) | **13 undispositioned at lines 810–822**, `id_counts = {'OQ': 13}` |
| negative control | same copy + ` BLOCKS_PLAN |` appended to each row | back to **0** |

Root cause, one line: `validate_plan.py:449` iterates `d.startswith("OPEN-") or d.startswith("OQ-")`.
`00-brief.md:810-822` names them `Q1`..`Q13`. **A two-character prefix mismatch turns a validated R3 gate
into a silent GREEN over 13 real undispositioned questions.** Same shape as R2's
`contract-law-ids-unregistered-a6b7` — content under an analogue naming scheme the register does not
admit — except here a checker runs and passes. → `omp-orchestrator-r3-oracle-exists-and-is-blind-54d3` (P0).

Two instruments, reconciled rather than averaged: my hand census of `00-brief.md` counts **14**
(11 `**OPEN**` + Q10 `**PARTIAL**` + K2/K5 `**OPEN/UNVERIFIED**`, excluding Q9 `**ANSWER MOVED**`); the
oracle counts **13** (all thirteen `Q` rows including Q9, and it never sees the two `K` rows because they
are not table ids). Union 15, intersection 12. The population total below uses the hand figure 14 and the
band is ±1.

## 8. PRODUCT

Filed this pass, each with WHAT/WHY/ACCEPTANCE, line-cited evidence, a named falsifier and an
anti-vacuity leg, `--actor AtlasArcPass3`:

| bead | P | finding |
|---|---|---|
| `omp-orchestrator-r3-two-registers-conflict-0gyc` | P0 | two R3 registers, same 177 gaps, 42/44 disagree, 92 s apart |
| `omp-orchestrator-r3-assume-reversibly-no-falsifier-tlae` | P0 | 41/45 (42/46 with maturity0) assumptions unfalsifiable |
| `omp-orchestrator-r3-oracle-exists-and-is-blind-54d3` | P0 | a validated R3 checker runs and returns 0 over 13 untyped questions |
| `omp-orchestrator-r3-defer-to-bead-names-no-bead-9bt4` | P1 | 83/88 deferrals name no bead; 11/12 name a person |
| `omp-orchestrator-r3-unknown-detector-is-empty-string-3zhb` | P1 | detector is `resolution == ""`; 20 open rows (4 blockers) invisible |
| `omp-orchestrator-r3-plan-sections-type-no-unknowns-yb4q` | P1 | 0 disposition tokens in 13 sections; 14 `**OPEN**` rows + HD-0019 untyped |
| `omp-orchestrator-r3-human-decision-names-no-gate-w73c` | P1 | 6 human decisions, 0 name a gate, 0 actuated |

Cited as tracked, not re-filed, each with a measurement comment: `omp-orchestrator-1qzt` (the seventh
class is already deployed unratified inside `OUT_OF_SCOPE`, and its insufficiency premise is refuted by a
sibling commit 92 s later), `omp-orchestrator-arc-typed-unknowns-02ai` (269 reproduces exactly; five
defects the census does not report), `omp-orchestrator-decision-ledger-needs-actuator-lef1` (394 unpushed
now; 0 of the 6 human decisions ever reach the ledger an actuator would drive),
`omp-orchestrator-cbp4` (10 dual-row pairs now, up from 6 filed / 8 at Pass 2).

## NO-CLAIM

This census does not resolve a single unknown, does not decide which of the two registers is canonical,
and does not certify `BUILD_READY` — `atlas-arc` ships no `scripts/arc.py` on this host, so **its** checker
is UNRUN, never a pass. The `planning-arc` substitution in §7 covers R3 only: the same file has 0 maturity
predicates (Pass 1, `r1-no-checker-d81g`) and resolves no owners (Pass 2). Per-disposition counts in the
contract line are the PRIMARY register's (`DISPOSITIONS.toml`), except `HUMAN_DECISION`, which is the union
of distinct objects across both registers (1 + 5) because §4 grades actuation over that union; the two
registers disagree on 42 of 44 alignable rows and no arithmetic reconciles them until `0gyc` picks one.

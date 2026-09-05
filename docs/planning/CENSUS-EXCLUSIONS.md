# Census exclusions for `docs/plan`

**Bead:** `omp-orchestrator-census-exclusions-specimens-and-foreign-ids-5dia`
**Measured:** 2026-09-05. Reviewer WildStone, pane `%9`. `BUILDS_RUN=0`.
**Victim today:** an agent doing a hand census. No in-repo gate harvests bead ids from `docs/plan` (`close-evidence-gate` harvests `bin/` and `.flywheel/` from bead comments).

## The rule (durable)

An exclusion *list* dies the way `DESIGN_INDEX.md` died: within an hour of being written, and a `FIXED_POINTER_ALLOWANCE` row already cited a commit that never touched its file. So the list below is a **floor**, not a closure. The rule is what a later census must obey even if this file is stale.

1. **Prefix.** This tracker's ids are `omp-orchestrator-*`. A `cp-*` id belongs to the control-plane tracker. Resolving it with this repo's `br` yields `ISSUE_NOT_FOUND` by prefix. That is not broken evidence and must not increment a dangling-citation count.
2. **Specimen prose.** An identifier that appears inside a `known-BAD` / `planted` / `supersedes` pointing at a nonexistent row / `fixture` sentence is a poison-pill for a gate's refusal leg, not a live register member. `HD-9[0-9]{3}` in that context is the sentinel shape. Keep the sentence; drop the id from any live-ID census.
3. **Refuse-literals.** `success:[N]`, `success:[4]`, `successful:[4]` are transport payloads a gate must refuse. They are not bead ids.
4. **Assembly is not a second population.** `docs/PLAN.md` is assembled from `docs/plan/*.md`, so an id counted in both is counted twice. Measured by the grader 2026-09-05: `PLAN.md` is 8,209 lines against 10,019 for `cat docs/plan/*.md`, so the two corpora **overlap and are not identical** — the inflation is real but is *not* a uniform 2×, and a census must pick one corpus rather than scale a combined count. `HD-9999` is a confirmed duplicate pair (`12-journey.md:471` and `PLAN.md:7577`).

**Dies when:** (a) this repo's bead prefix is no longer `omp-orchestrator-`, (b) a `cp-*` bead is actually filed *here*, (c) `HD-9xxx` is used as a real `docs/decisions.jsonl` row. Until then the rule does not need a new list.

**Does not die when:** a new `cp-*` or `HD-9999` sentence is added to the plan. The rule still classifies it. Only the floor enumeration below goes stale.

## Class 1 — planted known-bad specimens

These inflate a live-identifier population. They fail by counting poison-pills as members.

The 12→11 error: a `grep` over `docs/PLAN.md` reported 12 constitutional ids (`HD-0001`, `HD-9999`, `INV-2026`, `WP-001`..`WP-009`). `HD-9999` is the F4 known-bad at `12-journey.md:471`, copied to `PLAN.md:7577`. Real live set is 11 until GATE labels land.

| identifier / literal | class | why exclude | source `file:line` (do not also count `PLAN.md`) |
|---|---|---|---|
| `HD-9999` | specimen | F4 decision-ledger known-BAD: empty `decision` + `supersedes` to a nonexistent row. Gate: projected S9 ledger gate. Not in `docs/decisions.jsonl` (live rows are `HD-0001`..`HD-0018`, 18 rows, measured by the grader 2026-09-05 — this file's original `..HD-0008` was already stale by 10 rows when written, which is the rot the rule above predicts; the **rule** survived it because `HD-0009`..`HD-0018` are not the `HD-9[0-9]{3}` sentinel shape). | `docs/plan/12-journey.md:471` |
| `success:[4]` / `successful:[4]` / `success:[N]` | refuse-literal | Payload a receipt gate must refuse (send reports success, packet never lands). Not a bead id. Often adjacent to `cp-z42vu` — the *id* is Class 2; the *payload* is Class 1. | `05-actions.md:193-194`; `09-milestones.md:103,320`; `12-journey.md:326,339` |

Unnamed fixture *shapes* (no identifier to grep; listed so a later census does not mint one):

- S1 missing-`AGENTS.md` / identity-mismatch fixture — `12-journey.md:523`
- S2 bare-figure + UNKNOWN-without-experiment — `12-journey.md:583-584`
- S3 SEVERITY-removed / omitted `new_findings` / one-lens clean row — `12-journey.md:660-662`
- S4 two-bead cycle + blank acceptance — `12-journey.md:708`
- S7 exit-0 empty-digest transcript — `12-journey.md:407`

Do not invent ids for those shapes.

## Class 2 — foreign-repo citations

These manufacture a broken-reference count. They fail in the opposite direction from Class 1. Owning tracker: **control-plane** (`cp-*`). `br show` here → `ISSUE_NOT_FOUND` is expected.

`cp-z42vu` used as "known-BAD fixture PROJECTED" (`12-journey.md:339`) does **not** move it to Class 1. The foreign bead is real over there; the planted payload (`success:[4]`) is Class 1. Mixing the two is how `ISSUE_NOT_FOUND` gets read as fabrication.

| identifier | owning repo | why exclude from *this* `br` | source `file:line` |
|---|---|---|---|
| `cp-z42vu` | control-plane | Historical send-success-without-arrival incident. | `00-brief.md:493,526,530,534`; `01-idea.md:100`; `05-actions.md:194,219`; `09-milestones.md:103,116,320`; `10-prior-art.md:62,269`; `12-journey.md:322,326,339,349` (16 source hits) |
| `cp-3k9jq` | control-plane | 104-char close reason, zero path citations. Cited as the projected close-evidence known-BAD *shape*; the id is still foreign. | `05-actions.md:300`; `12-journey.md:361,375` (3 source hits) |
| `cp-nq2s9` | control-plane | `ntm --robot-send` "cod composer not visible" screen-state guard. | `09-milestones.md:111` (1 source hit) |

No other `cp-*` in `docs/plan/00-brief.md` … `12-journey.md`.

## Sweep ranges and residual

**Pattern-complete** over `docs/plan/{00-brief,01-idea,02-surface-census,03-crates,04-diagrams,05-actions,06-gates,07-installability,08-end-users,09-milestones,10-prior-art,11-lifecycle,12-journey}.md` for `HD-9[0-9]{3}`, `\bcp-[a-z0-9]+\b`, `success:[4]`, `successful:[4]`, `planted` / `known-BAD`.

**Not occurrence-mapped (floor, not closure):**

- `docs/PLAN.md` — assembled duplicate; same ids, different line numbers (`HD-9999` at `:7577`).
- `docs/plan/dag/**` — derived bead DAG JSON. Extra foreign id seen there: `cp-op5uu` (`dag-06.json`, pre-delete-citation specimen in control-plane). Not in the 13 section files.
- `docs/plan/FINDINGS.jsonl`, `docs/plan/round*.jsonl` — grader copies. They are why a whole-tree `docs/plan` grep reports `cp-z42vu` ≈ 60 and `cp-3k9jq` ≈ 8; the *unique* foreign ids do not grow.
- Review line-complete coverage of the plan is 7675/8057 (0.95). Identifier sweep is pattern-complete on the 13 sections, not a line-complete re-read of 00–05 / 06 remainder.

**This list is a floor.** A census that only subtracts the rows above and then claims a closed identifier population is repeating the 12→11 error with extra steps. Apply the rule.

## NO-CLAIM

Excluding these ids does not prove any other id in `docs/plan` is live, owned, or correctly cited. It only stops two measured miscounts.

## Rule 5 — a hex token in a close reason is not a git commit until `git cat-file -e` says so

**Bead:** `omp-orchestrator-classify-unresolvable-close-citations-bm04`. Measured 2026-09-05. `BUILDS_RUN=0`.

The 5dia rules classify *plan* identifiers. The same two poisons hit *close_reason* citations, plus two more:

5. **Object-kind.** `[0-9a-f]{7,40}` matches dates, byte counts, the word `succeeded` (`cceeded`), cargo crate revs, installed-binary shas, and **splits a 64-char sha256 into a 40-char prefix plus a 24-char remainder**. A token is a git commit in *this* repo only after `git cat-file -e <tok>` succeeds here. Adjacent `sha256`, `byte-identical`, `both sides`, or `…` means file digest, not commit. A hit in `/Users/josh/Developer/control-plane` is FOREIGN-REPO, not a broken citation.

Class 1 (FABRICATED) is the only lie in the audit trail. Class 2/4 are the census over-matching. Rebased-away was checked (`git reflog --all`, `git fsck --lost-found`) and was empty for this set.

**Dies when:** this repo's object database is rewritten such that `git cat-file -e` is no longer the authority, or control-plane shares history with this repo. Does not die when a new close reason cites a sha256.

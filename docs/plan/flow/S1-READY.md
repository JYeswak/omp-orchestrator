# S1-READY — the fh-backed criteria for when S1 may be built

**Purpose.** One canonical definition of "S1 is ready to build." Nothing else in this repo may
define it. Requested by Joshua 2026-09-07: *"we need to have specific fh backed criteria for when s1
is ready to build."*

**Bead:** `omp-orchestrator-gate-s1-djn8`

**Why ONE file.** `fh C47` (doctrine; content retrieved, **provenance UNAVAILABLE — `fh why C47`
returned `[DRIFT/LEDGER_SOURCE_DRIFT]`, refusing a partial trace**):

> WHEN DOCUMENTS PROJECT ONE IDENTITY-BEARING CONTRACT, REQUIRE EXACTLY ONE CANONICAL DEFINITION
> ACROSS THE TREE. Precedence prose cannot prevent [drift].

This repo already paid that: the S1 build authorization existed in `CONTRACT.md:101` while `:59`
said FREEZE, and the orchestrator told the fleet for a session that all code was blocked. **Two
places, one contract, and the reader obeyed the wrong one.** So `S1-READY` is defined here and
referenced elsewhere by path, never restated.

---

## Why planning must complete BEFORE building — the load-bearing row

`fh C35` (doctrine):

> *"Run the real product and diff its output" catches the defect in what you BUILT. It cannot catch
> having built the WRONG THING — that needs an unplanned question, and a question is not a fixture.
> Capture-and-diff is necessary and must not be sold as sufficient.*

**This is the whole argument.** Every S1 layer bead ships a writer, an artifact with readback, a
monitor, a gate with a known-bad leg, and a metric. All five are capture-and-diff instruments. **Not
one of them can tell us we built the wrong S1.** Joshua already made this exact call once — S1 was
flipped to `converged` at `a9ea672` and he retracted it: *"you can't claim converged when s1 is still
missing massive functionality."* Building faster does not answer that; a complete plan does.

`fh O6` (doctrine, `franken_tts`):

> A passing simulation or scorecard with zero observations has no non-vacuous oracle; reject it
> before a result can exist.

**Applied here:** an S1 readiness scorecard that no command computes is vacuous. Every criterion
below is a command, not a judgement.

---

## The criteria — R1..R8. S1 is READY when all eight pass.

Each row is `run X, expect Y`. **A criterion with no runner is not a criterion** (O6).

### R1 — every S1 layer bead carries acceptance
```bash
python3 - <<'PY'
import json,re
bad=[r['id'] for l in open('.beads/issues.jsonl') if l.strip().startswith('{')
     for r in [json.loads(l)] if r.get('id') and re.search(r'-s1-l[0-5]-',r['id'])
     and not (r.get('acceptance_criteria') or '').strip()]
print('S1_LAYER_BEADS_WITHOUT_ACCEPTANCE=%d' % len(bad)); print(*bad[:10],sep='\n')
PY
```
**Expect `=0`.** Measured 2026-09-07: **0 of 144.** ✅ PASSING.
*Rationale:* a bead with no acceptance is structurally undispatchable —
`crates/omp-orchestrator/src/dispatch_packet.rs:166` refuses the packet with
`PacketFieldMissing("acceptance")`.

### R2 — every S1 layer bead is WIRED to its layer gate
```bash
python3 - <<'PY'
import json,re
G={'omp-orchestrator-gate-s1-l%d-%s'%(i,t) for i,t in
   enumerate(['jtgw','fnv8','j5m9','z8hz','hs15','w44h'])}|{'omp-orchestrator-gate-s1-djn8'}
rows=[json.loads(l) for l in open('.beads/issues.jsonl') if l.strip().startswith('{')]
lay=[r for r in rows if r.get('id') and re.search(r'-s1-l[0-5]-',r['id'])]
wired={d.get('depends_on_id') for r in rows for d in (r.get('dependencies') or [])}|\
      {r['id'] for r in rows for d in (r.get('dependencies') or []) if (d.get('depends_on_id') or '') in G}
print('S1_LAYER_BEADS_UNWIRED=%d of %d'%(sum(1 for r in lay if r['id'] not in wired), len(lay)))
PY
```
**Expect `=0`.** Measured 2026-09-07: **144 of 144 unwired** — exactly **one** bead in the repo
*Rationale:* `fh N043` — BUILT ≠ WIRED. The authorization is live and nothing is attached to it.
**This is the single largest gap and it is pure planning work.**

### R3 — no S1 bead carries a `blocked` status without a real blocker
```bash
python3 -c "
import json,re
n=sum(1 for l in open('.beads/issues.jsonl') if l.strip().startswith('{')
      for r in [json.loads(l)] if r.get('id') and re.search(r'-s1-l[0-5]-',r['id'])
      and r.get('status')=='blocked' and not (r.get('dependencies') or []))
print('S1_FALSELY_BLOCKED=%d'%n)"
```
**Expect `=0`.** Measured 2026-09-07 21:5xZ: **89**. ❌ **FAILING.**
**This figure is a DATED SNAPSHOT, not a constant** — it read **92** twenty minutes earlier and fell
to 89 as `%20` reclaimed beads. **The runner is authoritative; every number in this file is a
timestamped snapshot — re-run before citing.** That discipline is here because this repo has
already been bitten three times by an assertion pinned to a live count: `docs-staleness`, the
`crate-atom-gate` ceilings, and `eg0m_jsonl_comment_count_is_seventeen` (asserts 17 against a live
27 — bead `325h`, whose acceptance explicitly FORBIDS re-pinning it to 27).
*Rationale:* `blocked` with an empty graph is `TrackerBlocked`, never `DependencyBlocked`
(`AGENTS.md`). Proven by transition, not by reading the field:
`br update omp-orchestrator-s1-l0-b01-3vro --status in_progress` → `blocked → in_progress`,
accepted, no refusal. **A false block hides work from `br ready` and from every selector.**

### R4 — the S1 box's disagreements are resolved, counted by a DERIVED predicate
```bash
grep -c 'DERIVED' docs/plan/flow/boxes/S1.toml   # expect >=1: the predicate is stated at the field
python3 -c "
import re,glob
u=0
for f in glob.glob('docs/plan/flow/waves/*/*.toml'):
    for b in open(f).read().split('[[disagreement]]')[1:]:
        m=re.search(r'''(?m)^\s*box\s*=\s*['\\\"]([^'\\\"]+)['\\\"]''',b)
        if not m or m.group(1)!='S1': continue
        r=re.search(r'''(?m)^\s*resolution\s*=\s*['\\\"]([^'\\\"]*)['\\\"]''',b)
        v=(r.group(1) if r else '')
        if not v.strip() or v.strip().startswith(('remaining','ACK_PARTIAL')): u+=1
print('S1_UNRESOLVED_ALL_WAVES=%d'%u)"
```
**Expect `=0` and the owner field's number to equal it.** Measured: **8 across all waves** (6 in
wave-2, +2 in wave-1); owner field reads `6` because its predicate is wave-2-scoped. ❌ **FAILING.**
*Rationale:* `%7` landed the derived predicate at `7b4ac63`; it must widen to all waves. **A
hardcoded `0` is not a measurement** — S2–S9 owner files each carry a literal `open_disagreements=0`
with no predicate at all (`S2:120 · S3:171 · S4:178 · S5a:135 · S5b:140 · S6a:137 · S6b:132 ·
S6c:137 · S7:165 · S8:175 · S9:132`).

### R5 — every S1 layer gate names its known-bad leg
```bash
for g in l0-jtgw l1-fnv8 l2-j5m9 l3-z8hz l4-hs15 l5-w44h; do
  printf '%s ' "$g"
  br --lock-timeout 60000 show "omp-orchestrator-gate-s1-$g" --json \
    | python3 -c "import json,sys;b=json.load(sys.stdin)[0];a=b.get('acceptance_criteria') or '';print('known-bad' if 'known-bad' in a else 'MISSING')"
done
```
**Expect `known-bad` × 6.** UNMEASURED — the six gate titles all read *"gate fired on known-bad"*,
so this likely passes, but **the title is not the acceptance field** and I have not read all six.
*Rationale:* `AGENTS.md` — a gate that has never fired on a bad input is not evidence of anything;
an attack-only suite ships an over-strict gate.

### R6 — the box's diagram receipt matches a fresh validator run
```bash
frankenmermaid validate docs/plan/flow/diagrams/S1.mmd --fail-on warning
grep -E 'validate_exit|nodes|edges' docs/plan/flow/boxes/S1.toml
```
**Expect exit 0 and the checked-in `nodes`/`edges` to equal the run.** Measured: **PASSING** after
`f6bbc1c` corrected a stale `52/60` to a re-run `53/64`, and added `validate_command`. ✅
*Rationale:* a `validate_exit = 0` recorded from an earlier revision is a fooled certificate.

### R7 — S0 is closed
```bash
for t in n7yp zey6 jplf.1.1 jplf.1.2 jplf.7.2; do
  printf '%s ' "$t"; br --lock-timeout 60000 show "omp-orchestrator-$t" --json \
    | python3 -c "import json,sys;print(json.load(sys.stdin)[0].get('status'))"
done
br --lock-timeout 60000 show omp-orchestrator-s0-asupersync-misapplication-audit-kvsq --json \
  | python3 -c "import json,sys;print('epic:',json.load(sys.stdin)[0].get('status'))"
```
**Expect all `closed`.** Measured: `n7yp` closed, `zey6` closed, `jplf.1.2` closed, **`jplf.1.1`
open (P1)**, **`jplf.7.2` open (P0)**, epic `kvsq` **open**. ❌ **FAILING — 3 of 5.**
*Rationale:* S0 is the asupersync-correctness floor S1 stands on. Building S1 layers on an unproven
cancellation contract means every layer inherits an unmeasured defect. `%20` just filed `62lz`
(unbounded `.output()` on the commit path) — a live S0-class violation found *while* S1 was
authorized.

### R8 — no criterion above rests on a stale or self-referential instrument
```bash
fh health          # staleness is part of every citation
git status --porcelain | wc -l     # worktree dirt: cargo reads the WORKTREE, a sha names a TREE
git worktree list | wc -l          # policy: exactly 1
```
**Expect** `fh health` GREEN, worktree count `1`, and every figure above labelled with the tree it
came from. Measured: `fh health` **RED — `digest_missing_today`, the harvest did not run today**;
**3 worktrees** (policy is 1), one under `/private/tmp` which `scratch-home` forbids for durable
state; **103 dirty files**. ❌ **FAILING.**
*Rationale:* seven instrument errors were recorded in one session — a triple-counting `dmesg`
alternation, a quote-naive `box =` regex that invented 12 phantom rows, a `capture-pane -S -400`
paged window read as absence, a homoglyph (`l0`→`10`) that emptied an authorized set, a stripped
tool legend, `ps -p A B` without a comma, and a wrong-scope filename glob. **The instrument produced
the reading, not the subject, every time.**

---

## Scorecard — measured 2026-09-07

|criterion|state|
|---|---|
|R1 acceptance on every layer bead|✅ **PASS** — 0 of 144 empty|
|R2 layer beads wired to their gate|❌ **FAIL** — 144 of 144 unwired|
|R3 no false `blocked`|❌ **FAIL** — 89 (snapshot; was 92)|
|R4 disagreements resolved, derived count|❌ **FAIL** — 8 open; owner says 6|
|R5 every gate names a known-bad leg|⚠ **UNMEASURED** — read all six acceptance fields|
|R6 diagram receipt matches a fresh run|✅ **PASS** — 53/64, exit 0|
|R7 S0 closed|❌ **FAIL** — 3 of 5; epic open|
|R8 instruments not stale/self-referential|❌ **FAIL** — fh RED, 3 worktrees|

**S1 IS NOT READY TO BUILD. 2 of 8 pass, 1 unmeasured, 5 fail.**

**Every failing criterion is planning or hygiene work — none needs new product code.** R2, R3, R4
are tracker and predicate work. R7 is two beads. R8 is a stale harvest and a worktree prune. That is
the answer to *"plan S1 fully before executing"*: the plan is not short of ideas, it is short of
**wiring, honest counts, and a closed floor.**

## NO-CLAIM

This file defines readiness; it does not confer it. Eight passing criteria mean the **plan** is
coherent and countable — per `fh C35` they still cannot prove we planned the *right* S1, which is why
Joshua's approval row remains a separate and irreducible gate. R5 is UNMEASURED and must not be read
as passing. All `fh` rows here were retrieved while `fh health` reported **RED
(`digest_missing_today`)**, and `fh why C47` refused its trace with `LEDGER_SOURCE_DRIFT` — so C47 is
cited for content, not provenance.

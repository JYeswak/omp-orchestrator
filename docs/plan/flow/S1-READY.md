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
import json, re
rows = [json.loads(l) for l in open('.beads/issues.jsonl') if l.strip().startswith('{')]
by = {r['id']: r for r in rows if r.get('id')}
G = {'omp-orchestrator-gate-s1-l%d-%s' % (i, t) for i, t in
     enumerate(['jtgw', 'fnv8', 'j5m9', 'z8hz', 'hs15', 'w44h'])} | {'omp-orchestrator-gate-s1-djn8'}
# SURFACE MATTERS. In .beads/issues.jsonl a dep record keys on `depends_on_id`/`type`.
# `br show --json` keys the SAME edge on `id`/`dependency_type`. Using br's keys here
# silently yields an empty wired-set and reports EVERY bead unwired.
wired = {d.get('depends_on_id') for g in G
         for d in (by.get(g, {}).get('dependencies') or []) if d.get('depends_on_id')}
# POPULATION EXCLUDES THE GATES THEMSELVES: the id regex matches gate-s1-l0..l5, and a
# gate can never be wired to its own gate, so including them pins a correct R2 above zero.
lay = [r['id'] for r in rows
       if r.get('id') and re.search(r'-s1-l[0-5]-', r['id']) and r['id'] not in G]
print('S1_LAYER_BEADS_UNWIRED=%d of %d' % (sum(1 for i in lay if i not in wired), len(lay)))
PY
```
**Expect `=0`.** Measured 2026-09-07 22:3xZ: **0 of 138.** ⚠ **PASSING BUT THE PASS IS WEAK — see
`omp-orchestrator-2fxd` (P0).**

> ⛔ **THIS CRITERION HAS BEEN WRONG TWICE, IN OPPOSITE DIRECTIONS. Both were mine.**
>
> **First publication: FAIL, "144 of 144 unwired."** Wrong.
> **First "correction": switched the runner to `id`/`dependency_type`.** Also wrong, and *worse* —
> those are `br show --json`'s keys, not the JSONL's. Measured on both surfaces:
>
> ```
> .beads/issues.jsonl  dep keys : created_at created_by depends_on_id issue_id metadata thread_id type
> br show --json       dep keys : dependency_type id priority status title
>
> executed against the JSONL:  depends_on_id -> 259 targets, unwired 0 of 144   CORRECT
>                              id            -> 0 targets,   unwired 144 of 144  WRONG
> ```
>
> **`%20` caught it:** *"anyone who 'fixes' S1-READY.md by switching to id/dependency_type will
> break a runner that currently works — those keys do not exist in the JSONL."* My interactive
> check only passed because it carried a fallback (`d.get('id') or d.get('depends_on_id')`) that I
> never wrote into the file, so the **documented** runner was live-broken while I reported it fixed.
> **A correction that ships an untested runner is worse than the defect it replaces.**
>
> **And the snapshot defence does not apply.** `%20` dated the 228 gate→bead edges to
> **2026-09-03T18:33–19:26** (creators: josh 122, WildStone 96, pane1 35) — **four days before this
> file existed.** The 144 was false at publication, not aged into falsehood.

**THE PASS IS WEAK AND MUST NOT BE BANKED — three defects, filed as `omp-orchestrator-2fxd` (P0):**

1. **Population included the gates.** `-s1-l[0-5]-` matches `gate-s1-l0-jtgw … l5-w44h`, so the
   denominator was 138 beads **+ 6 gates**, and a gate cannot be wired to itself — those six would
   hold a *correct* R2 above zero forever. Excluded above; denominator is now **138**.
2. **The predicate never required GATE linkage.** The earlier `wired` set unioned **every**
   dependency target in the tracker, so a bead counted as wired if anything anywhere depended on
   it. **LATENT, not active** — `%20` measured 0 of 138 rows exploiting it — but **R2's PASS
   therefore carries no information about gate linkage**, which is why this row reads ⚠ and not ✅.
3. **THE DIRECTION TRAP.** R2's stated remedy — *"wire 144 beads to depend on their gates"* — is
   the strangling pattern **at 144× scale**. `%20` refused to execute it, wired the one genuinely
   unwired bead (`s1-l3-blocked-on-frozen-crate-tjxt`) in the safe direction, and ran the falsifier
   instead. **`br dep add <gate> <bead>` keeps the bead claimable; the transpose bricks it.**

*Rationale:* `fh N043` — BUILT ≠ WIRED. The edges exist. **What remains unproven is whether any gate
FIRES, which is R5.**

### R3 — no S1 bead carries a `blocked` status without a real blocker
```bash
python3 -c "
import json,re
n=sum(1 for l in open('.beads/issues.jsonl') if l.strip().startswith('{')
      for r in [json.loads(l)] if r.get('id') and re.search(r'-s1-l[0-5]-',r['id'])
      and r.get('status')=='blocked' and not (r.get('dependencies') or []))
print('S1_FALSELY_BLOCKED=%d'%n)"
```
**Expect `=0`.** Measured 2026-09-07 22:0xZ: **2**. ⚠ **NEARLY PASSING.**
**This figure is a DATED SNAPSHOT, not a constant** — it read **92**, then **89**, then **88**, then **2** inside one hour as `%20` converted them. **The runner is authoritative; every number in this file is a
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

### R5 — every S1 layer gate names its known-bad leg AND has fired

```bash
for g in l0-jtgw l1-fnv8 l2-j5m9 l3-z8hz l4-hs15 l5-w44h djn8; do
  printf '%s ' "$g"
  br --lock-timeout 60000 show "omp-orchestrator-gate-s1-$g" --json \
    | python3 -c "import json,sys;a=(json.load(sys.stdin)[0].get('acceptance_criteria') or '');\
print(len(a), 'known-bad' if 'known-bad' in a.lower() else 'NO-KNOWN-BAD')"
done
grep -ci 's1-l' .github/workflows/gate.yml   # expect >0 if any layer gate runs in CI
crontab -l 2>/dev/null | grep -ci 's1-l'      # expect >0 if any layer gate runs on a timer
```
**Expect a known-bad leg on all seven AND a nonzero invocation count.** Measured 2026-09-07 22:4xZ:

```
gate         acc    known-bad   asserts a MESSAGE
l0-jtgw      3625   YES         YES
l1-fnv8      3625   YES         YES
l2-j5m9      3625   YES         YES
l3-z8hz      3625   YES         YES
l4-hs15      3625   YES         YES
l5-w44h      3625   YES         YES
djn8         4728   YES         no          <- the stage gate does not assert a message

.github/workflows/gate.yml   14 jobs, 0 naming s1-l
crontab -l                   0 naming s1-l
```

❌ **FAILING — and it was mis-labelled UNMEASURED, which hid the remedy.**

**All seven SPECIFY a known-bad leg. NONE HAS EVER FIRED.** Per gate rule 4a the classification is
**INERT** — the gates exist, are wired as bead dependencies, appear in `docs/contracts/`
(`s1_l0_install.md:159`, `s1_l3_walkthrough.md:148`, `s1_l4_liveness.md:156`,
`s1_l5_portal.md:150`), and **nothing invokes any of them.** `gate.yml`'s 14 jobs are
`no-shell-gate`, `path-literal-guard`, `kernel-bypass-gate`, `state-wildcard-lint`,
`undrained-pipe-lint`, `grader-attribution-gate`, `installer`, `pre-delete-citation-check`,
`head-compiles-as-committed` and peers — **not one S1 layer gate.**

**INERT means WIRE IT, not write it.** That distinction is the whole value of this measurement:
"UNMEASURED" invited someone to go author known-bad legs that already exist at 3,625 chars each.

**POSITIVE CONTROL on the detection method** (mandatory — a method that finds nothing must be shown
capable of finding something): the pre-commit multi-gate **does** fire and the same method sees it —
`gate.yml:449-450` invokes `pre-delete-citation-check`, and it was observed firing three times
today (`exit 3 NOTHING_TO_CHECK` on an empty index, `exit 1 VIOLATION` naming
`pre-delete-citation-check` on a deletion-only fixture, and RED under a peer's mutation). **Those
are a DIFFERENT gate family and must never be miscited as R5 progress** — a peer measured them and
refused exactly that inference.

**Residual on `djn8`:** its 4,728-char acceptance names a known-bad leg but does **not** assert a
message. `101` is cargo's generic failure and an unrelated workspace-loading error produced an
identical `101` in this repo, so a leg keyed on `rc != 0` goes green on unrelated breakage. Fix
before djn8 is relied on as the stage gate.

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
python3 - <<'PY'
import json
EPIC='omp-orchestrator-s0-asupersync-misapplication-audit-kvsq'
rows=[json.loads(l) for l in open('.beads/issues.jsonl') if l.strip().startswith('{')]
by={r['id']:r for r in rows if r.get('id')}
kids={d['id'] for d in (by.get(EPIC,{}).get('dependencies') or []) if d.get('id')}
kids|={r['id'] for r in rows if (r.get('id') or '').startswith('omp-orchestrator-s0-')
       and r['id']!=EPIC}
open_=[k for k in kids if by.get(k,{}).get('status')!='closed']
print('S0_EPIC=%s  CHILDREN=%d  OPEN=%d'%(by.get(EPIC,{}).get('status'),len(kids),len(open_)))
for k in sorted(open_):
    print('  ',by[k]['status'],'P%s'%by[k].get('priority'),k.replace('omp-orchestrator-',''))
PY
```
**Expect `S0_EPIC=closed` and `OPEN=0`.** Measured 2026-09-07 22:1xZ:
**epic `kvsq` OPEN, 7 children, 4 OPEN — all P0.** ❌ **FAILING.**

```
in_progress  P0  s0-audit-checkpoint-density-xv5r
grading      P0  s0-audit-detached-spawn-w21v
in_progress  P0  s0-audit-scope-regions-zaxp
in_progress  P0  s0-unblock-messaging-fabric-feature-jix1
```

> ⛔ **SCOPE CORRECTED.** This criterion first listed `n7yp zey6 jplf.1.1 jplf.1.2 jplf.7.2` and
> reported "3 of 5". **`jplf.1.1` and `jplf.7.2` are not S0 children** — `jplf.1.1` sources from
> `docs/plan/05-actions.md:L253-L262` and `jplf.7.2` from `docs/plan/07-installability.md:L113-L116`,
> and neither appears in the epic's dependency set. Counting them made S0 look 60% done against a
> denominator that was partly another stage's work. **The runner now derives the child set from the
> epic instead of a hand-typed list**, which is the same fix R2 needed: stop hand-listing what a
> query can enumerate.
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
|R2 layer beads wired to their gate|⚠ **WEAK PASS** — 0 of 138; predicate does not require gate linkage (`2fxd` P0). Published FAIL **and** its first correction were both wrong|
|R3 no false `blocked`|⚠ **NEARLY** — 2 (snapshot; was 92 → 89 → 88 → 2)|
|R4 disagreements resolved, derived count|❌ **FAIL** — 8 open; owner says 6|
|R5 gates name a known-bad leg AND fire|❌ **FAIL** — 7/7 specify one (3625 ch each; djn8 4728); **0 of 7 have ever fired**, 0 in CI, 0 in cron → **INERT: wire it, do not write it**. `djn8` asserts no message|
|R6 diagram receipt matches a fresh run|✅ **PASS** — 53/64, exit 0|
|R7 S0 closed|❌ **FAIL** — epic open; 4 of 7 children open, all P0 (**scope corrected: was mis-counted as 3 of 5**)|
|R8 instruments not stale/self-referential|❌ **FAIL** — fh RED, 3 worktrees|

**S1 IS NOT READY TO BUILD. 2 PASS · 2 WEAK/NEARLY · 4 FAIL. Nothing is UNMEASURED any more.**

**CORRECTED 2026-09-07 22:0xZ.** As first published this read **2/8 with R2 FAIL**. R2 was a
**wrong-key artifact** and is retracted at the criterion. R3 fell 92 → 2 under `%20`. The
remaining real failures are **R7** (S0 epic open, 4 P0 children), **R8** (instruments stale), and **R4** (8
disagreements) — plus **R5 now MEASURED as FAIL: the edges and the known-bad legs both exist; no gate has ever
fired. INERT, not unwritten.**

**Every failing criterion is planning or hygiene work — none needs new product code.** R2, R3, R4
are tracker and predicate work. R7 is two beads. R8 is a stale harvest and a worktree prune. That is
the answer to *"plan S1 fully before executing"*: the plan is not short of ideas, it is short of
**wiring, honest counts, and a closed floor.**

## What actually gets us to green — measured 2026-09-07 22:4xZ by four read-only scouts

**Every remaining failure is wiring, counting, or hygiene. NOT ONE needs new product code.**

|criterion|the single action that moves it|owner|
|---|---|---|
|**R5**|**Wire the seven gates into `gate.yml`.** Their known-bad legs already exist at 3,625 chars each. INERT, not unwritten. Separately: give `djn8` a message assertion|CI lane|
|**R7**|Close S0's four P0 children, then the epic. **Read `kvsq`'s own 3,060-char acceptance first** — it carries four closing conditions and a known-bad leg, and `deps=0`, so no dependency edge encodes them|the four holders|
|**R4**|8 rows: 6 in wave-2 `pane1-S1.toml` (five explicitly "stays open"/"escalated"), 2 in `wave-1/SilverWolf-S1.toml` with blank resolutions. **Decide first whether wave-1 is in R4's scope** — the criterion carries no wave qualifier and the owner field is wave-2-scoped, which is the entire 8-vs-6 gap|pane 1 + Joshua|
|**R8**|`fh` harvest has **no cron/launchd entry** — running it is a read-only scan of 216 frozen mirror repos, safe. Prune 2 worktrees **only after** confirming no live process. 106 dirty across 5 writers — attribute, don't judge|infra|
|**R2**|`2fxd`: make the predicate require gate linkage rather than any inbound edge|pane 20|
|**R3**|Add the third state. `AwaitingHumanDecision` has no representation, so the last 2 rows keep R3 permanently nonzero and **flipping them would falsify rows to zero a number**|pane 1|

### THE S0 REFRAME — read this before auditing anything under it

`kvsq`'s own acceptance already retracts the figure its description carries, and it changes what the
epic *is*:

> *"The description says '20 raw thread::spawn'. That is FALSE and is retracted… `std::thread::spawn`
> is a SUBSTRING of `thread::spawn`, so 14+6=20 double-counted. Proven by `comm -13` returning 0."*

Its measured decomposition: **14 grep matches → 8 production sites** (2 lint string literals, 2 doc
comments, 2 under `#[cfg(test)]`), and **of the 8: 5 JOINED, 3 detached-and-defensible** — the
`tick-monitor:123` waiter captures its pid at `:113` *before* the child moves and group-kills on
`recv_timeout` expiry; `loop-driver:777`'s wall watchdog must outlive its work by design.

> **"ZERO orphan-producing detached spawns exist in production."** The asupersync contract — drain
> both pipes on dedicated threads before the wait, capture pid before the move, GROUP kill, join
> readers against `READER_JOIN_GRACE` — **is already implemented BY HAND and correctly.**
>
> **"THEREFORE the epic is a LEVERAGE finding, not a correctness one… we hand-roll exactly what
> `Scope` and `JoinSet` provide and consume neither."**

**So the "7 asupersync surfaces used ZERO times" census is not a defect list.** `kvsq` condition 2
requires every recommendation to state what the runtime surface buys **beyond** the hand-rolled
code, and rules that **"'Nothing but uniformity' is an ACCEPTED verdict and is preferred over an
invented defect."**

**Tested against that condition, one finding survives and it is worth naming:** the three unbounded
`.output()` calls in the resident supervisor — `df -k` (`main.rs:4077`), `shasum -a 256`
(`resident_tick.rs:466`), `lsof -nP +D` (`target_directory.rs:318`) — have **no deadline at all**,
unlike the 8 spawn sites which are joined or defensibly detached. Bounding them buys a typed
restrictive terminal where today there is an indefinite block, so it clears condition 2 on its
merits rather than on uniformity.

**And `kvsq` ships its own known-bad leg, aimed at us:** *"this acceptance must FAIL for any slice
report whose headline figure equals a number copied from this description rather than re-measured.
That is the failure this bead just demonstrated on itself."* Two of the four slice titles
(`w21v`'s "20 raw", `zaxp`'s "ZERO files") carry exactly such inherited figures — **their acceptance
now forces re-derivation, which is why the drift is a first finding rather than a blocker.**

### Scout instrument note, recorded because it repeated

Two of the four scouts **banked a stale read despite being warned in the same broadcast**:
`R7S0Closure` reported `w21v` `acceptance_criteria=0` from `.flywheel/o3eb-w21v-grade-evidence.md`
— a stale artifact — while the DB held **4,710** chars, and attributed `w21v`'s title to `xv5r`.
`R4Disagreements` cited `CONTRACT.md:56-57` as calling wave-1 a "SURVEY" (those lines are the
cross-review step and the Wave-1 addendum; the claim is **unsupported**) and reported "ten raw
`Command::new(ntm)` sites across nine crates" where the measured count is **1 site in 1 crate**
(`kernel-bypass-gate`), confirmed across seven pattern variants. **A written warning did not prevent
the class; only reading the authoritative surface does.** `R5GateFiring` by contrast returned
UNKNOWN wherever it lacked `br` access rather than guessing — that is the behaviour to copy.

## NO-CLAIM

This file defines readiness; it does not confer it. Eight passing criteria mean the **plan** is
coherent and countable — per `fh C35` they still cannot prove we planned the *right* S1, which is why
Joshua's approval row remains a separate and irreducible gate. R5 is UNMEASURED and must not be read
as passing. All `fh` rows here were retrieved while `fh health` reported **RED
(`digest_missing_today`)**, and `fh why C47` refused its trace with `LEDGER_SOURCE_DRIFT` — so C47 is
cited for content, not provenance.

# S1-READY — the fh-backed criteria for when S1 may be built

**Purpose.** One canonical definition of "S1 is ready to build." Nothing else in this repo may
define it. Requested by Joshua 2026-09-07: *"we need to have specific fh backed criteria for when s1
is ready to build."*

**Bead:** `omp-orchestrator-gate-s1-djn8`
**Done definition:** S1 readiness is distinct from S1 completion. S1 is DONE only when
omp-orchestrator-gate-s1-djn8 is independently graded closed under its acceptance; the
criteria below authorize building, but do not claim that S1 has been built or completed.
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
# R2_JSONL lets the SAME runner be pointed at a fixture; it defaults to the live tracker.
R2_JSONL=${R2_JSONL:-.beads/issues.jsonl} python3 - <<'PY'
import json, os, re, sys
path = os.environ['R2_JSONL']
rows = [json.loads(l) for l in open(path) if l.strip().startswith('{')]
by = {r['id']: r for r in rows if r.get('id')}
G = {'omp-orchestrator-gate-s1-l%d-%s' % (i, t) for i, t in
     enumerate(['jtgw', 'fnv8', 'j5m9', 'z8hz', 'hs15', 'w44h'])} | {'omp-orchestrator-gate-s1-djn8'}
# SURFACE MATTERS. In .beads/issues.jsonl a dep record keys on `depends_on_id`/`type`.
# `br show --json` keys the SAME edge on `id`/`dependency_type`. Using br's keys here
# silently yields an empty wired-set and reports EVERY bead unwired.
DEPS = lambda i: (by.get(i, {}).get('dependencies') or [])
# POPULATION EXCLUDES THE GATES THEMSELVES: the id regex matches gate-s1-l0..l5, and a
# gate can never be wired to its own gate, so including them pins a correct R2 above zero.
lay = [r['id'] for r in rows
       if r.get('id') and re.search(r'-s1-l[0-5]-', r['id']) and r['id'] not in G]
def own_gate(i):
    return 'omp-orchestrator-gate-s1-l%s-%s' % (
        re.search(r'-s1-l([0-5])-', i).group(1),
        ['jtgw', 'fnv8', 'j5m9', 'z8hz', 'hs15', 'w44h'][int(re.search(r'-s1-l([0-5])-', i).group(1))])
# GATE LINKAGE IS REQUIRED, and ONLY its own layer gate counts. The retired predicate
# unioned EVERY dependency target in the tracker, so any inbound edge from anywhere read
# as "wired" -- see the fires-on-known-bad leg below, where the two answers differ.
held = {i: i in {d.get("depends_on_id") for d in DEPS(own_gate(i))} for i in lay}
# DIRECTION IS PART OF THE PREDICATE. gate->bead keeps the bead claimable; the transpose
# is the strangling pattern and must be an ERROR, never a way to satisfy this criterion.
strangled = [i for i in lay if own_gate(i) in {d.get('depends_on_id') for d in DEPS(i)}]
unwired = [i for i in lay if not held[i] and i not in strangled]
# ANTI-VACUITY: an empty layer set or an absent gate is an ERROR, never a pass.
if not lay:
    print('S1_LAYER_BEADS_UNWIRED=ERROR empty_layer_population surface=%s' % path); sys.exit(1)
if not (G & set(by)):
    print('S1_LAYER_BEADS_UNWIRED=ERROR no_gate_rows_present surface=%s' % path); sys.exit(1)
print('S1_LAYER_BEADS_UNWIRED=%d of %d  STRANGLED=%d  surface=%s'
      % (len(unwired), len(lay), len(strangled), path))
# The retired predicate, printed beside the corrected one so the difference is the evidence.
any_inbound = {d.get('depends_on_id') for r in rows for d in (r.get('dependencies') or [])}
print('RETIRED_PREDICATE_any_inbound_edge=%d of %d'
      % (sum(1 for i in lay if i not in any_inbound), len(lay)))
if strangled:
    print('STRANGLED_IDS=%s' % ','.join(strangled)); sys.exit(1)
sys.exit(1 if unwired else 0)
PY
```
**Expect `=0` and `STRANGLED=0`.** Measured 2026-09-07: **0 of 138, STRANGLED=0.** ✅ **PASSING —
`omp-orchestrator-2fxd` (P0) landed the predicate, the direction check, the anti-vacuity arm and the
fixture legs. The retired predicate is printed beside the corrected one on every run, so the two can
never silently converge again.**

**FIRES-ON-KNOWN-BAD, and the difference between the two answers IS the defect.** Four fixtures under
`docs/fixtures/`, each a `.jsonl` the runner above reads through `R2_JSONL`:

```bash
for f in unwired wired strangled empty; do
  printf '%-10s ' "$f"
  R2_JSONL=docs/fixtures/r2-$f.jsonl <the runner above>; echo "  rc=$?"
done
```

|fixture|what it holds|corrected|retired|rc|
|---|---|---|---|---|
|`r2-unwired`|a layer bead depended on ONLY by a **non-gate** row|**1 of 1 UNWIRED**|**0 of 1** — calls it wired|1|
|`r2-wired`|the **gate** depends on the bead (safe direction)|0 of 1|0 of 1|0|
|`r2-strangled`|the **bead** depends on its own gate (transpose)|`STRANGLED=1` + `STRANGLED_IDS=`|`1 of 1` — flags it unwired and CANNOT SAY WHY|1|
|`r2-empty`|zero S1 layer beads|`ERROR empty_layer_population`|would print `0 of 0`|1|

Every row above is executed output, not predicted. Two of the four are legs that could not exist
before:

* **`r2-unwired`** — the retired predicate **cannot** report it, because a single inbound edge from
  any non-gate row satisfied it. This is defect 2 made visible.
* **`r2-strangled`** — the retired predicate calls it `1 of 1` unwired, which is *directionally
  right and diagnostically useless*: the edge exists, it is simply transposed, so an operator
  following that output would **add the very edge that is already there** and leave the bead
  bricked. The corrected runner names the id and exits 1 on `STRANGLED`, so **the remedy that would
  brick S1 can no longer be scored as compliance, nor mistaken for a missing edge.**

> **A KNOWN-GOOD LEG CAUGHT THIS RUNNER'S OWN BUG BEFORE IT SHIPPED, and that is the argument for
> making one mandatory.** The first version of the predicate above tested
> `own_gate(i) in DEPS(own_gate(i))` — the wrong variable — and reported **`138 of 138` unwired on
> the live tracker**, which is *precisely* the false FAIL this criterion has already published once.
> Every attack leg (`r2-unwired`, `r2-strangled`, `r2-empty`) passed happily with that bug in place;
> only `r2-wired`, the leg that asserts a CORRECT input still succeeds, went red. An attack-only
> suite would have shipped it.

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

**THE THREE DEFECTS, ALL NOW CLOSED BY `omp-orchestrator-2fxd` (P0) — kept here because a reader
must be able to tell a fixed criterion from a fixed number:**

1. **Population included the gates.** `-s1-l[0-5]-` matches `gate-s1-l0-jtgw … l5-w44h`, so the
   denominator was 138 beads **+ 6 gates**, and a gate cannot be wired to itself — those six would
   hold a *correct* R2 above zero forever. Excluded above; denominator is now **138**.
2. **The predicate never required GATE linkage.** ~~The earlier `wired` set unioned **every**
   dependency target in the tracker, so a bead counted as wired if anything anywhere depended on
   it.~~ **FIXED.** The predicate now tests membership in *its own layer gate's* dependency targets
   and nothing else, and every run prints the retired predicate beside it so the two cannot
   silently converge again. The defect was **LATENT, not active** — 0 of 138 rows exploited it — so
   the figure never moved; what changed is that the PASS now carries information. `r2-unwired` is
   the fixture that makes the difference observable.
3. **THE DIRECTION TRAP.** R2's stated remedy — *"wire 144 beads to depend on their gates"* — is
   the strangling pattern **at 144× scale**. `%20` refused to execute it, wired the one genuinely
   unwired bead (`s1-l3-blocked-on-frozen-crate-tjxt`) in the safe direction, and ran the falsifier
   instead. **`br dep add <gate> <bead>` keeps the bead claimable; the transpose bricks it.**
   **NOW MECHANICAL, not advisory:** a bead whose own dependency list names its layer gate is
   reported as `STRANGLED` with its id and exits 1. The transpose can no longer be scored as
   compliance — and, per the `r2-strangled` row above, it can no longer be mistaken for a *missing*
   edge either, which is the mistake the retired predicate's output invited.

*Rationale:* `fh N043` — BUILT ≠ WIRED. The edges exist and the predicate now proves they are
**gate** edges in the **safe** direction. **What remains unproven is whether any gate ever FIRES,
which is R9 — not R5.** R5 asks whether each gate NAMES a known-bad leg and passes 7 of 7
(`omp-orchestrator-8hq3`); firing is a separate criterion with a separate runner, and conflating
the two is what made R5 read FAIL for an hour.

### R3 — no S1 bead carries a `blocked` status without a real blocker
```bash
# R3_JSONL / R3_DECISIONS point the SAME runner at fixtures; both default to live.
R3_JSONL=${R3_JSONL:-.beads/issues.jsonl} \
R3_DECISIONS=${R3_DECISIONS:-docs/decisions.jsonl} python3 - <<'PY'
import json, os, re, sys
beads, decisions = os.environ['R3_JSONL'], os.environ['R3_DECISIONS']
rows = [json.loads(l) for l in open(beads) if l.strip().startswith('{')]
# ANY-ROW DECISION RESOLUTION. HD-0009..HD-0014 each have an EMPTY FIRST ROW followed by a
# decision of up to 3,056 chars, so a `decision == ""` scan re-asks answered questions.
# This mirrors crates/decision-ledger (`empty_first_row_does_not_mask_nonempty_decision`,
# regression floor a8e2fc6) -- that kernel is the single source; see the NO-CLAIM below.
decided = set()
seen_ids = set()
for l in open(decisions):
    if not l.strip().startswith('{'):
        continue
    d = json.loads(l)
    hid = d.get('id') or d.get('hd') or d.get('decision_id') or ''
    # AN HD ID MUST LOOK LIKE ONE. Without this, ANY row carrying an `id` registers as a
    # decision id -- a bead file passed as the ledger yielded bogus ids and SILENTLY
    # DISARMED the anti-vacuity guard below. Caught by r3's own fixture run.
    if not re.fullmatch(r'HD-\d{4}', hid):
        continue
    if hid:
        seen_ids.add(hid)
        if (d.get('decision') or '').strip():
            decided.add(hid)
# POPULATION EXCLUDES THE SIX LAYER GATES, exactly as R2 does. Two criteria over one set must
# not disagree on the denominator: with the gates in, this printed 144 where R2 printed 138.
GATES = {'omp-orchestrator-gate-s1-l%d-%s' % (i, t) for i, t in
         enumerate(['jtgw', 'fnv8', 'j5m9', 'z8hz', 'hs15', 'w44h'])} | {'omp-orchestrator-gate-s1-djn8'}
lay = [r for r in rows
       if r.get('id') and re.search(r'-s1-l[0-5]-', r['id']) and r['id'] not in GATES]
blocked = [r for r in lay if r.get('status') == 'blocked']
cls = {'DependencyBlocked': [], 'TrackerBlocked': [], 'ExecutionOwed': [],
       'AwaitingHumanDecision': [], 'Unclassifiable': []}
for r in blocked:
    if r.get('dependencies'):
        cls['DependencyBlocked'].append(r['id']); continue
    blob = ' '.join(str(r.get(k) or '') for k in ('title', 'description', 'acceptance_criteria'))
    hds = sorted(set(re.findall(r'HD-\d{4}', blob)))
    if not hds:
        cls['TrackerBlocked'].append(r['id'])
    elif any(h in decided for h in hds):
        cls['ExecutionOwed'].append(r['id'])
    elif all(h in seen_ids for h in hds):
        cls['AwaitingHumanDecision'].append(r['id'])
    else:
        cls['Unclassifiable'].append(r['id'])
# ANTI-VACUITY, both arms. An unreadable ledger and an empty blocked set are DIFFERENT facts
# from "no false blocks", and neither may read as a pass.
if not seen_ids:
    print('S1_FALSELY_BLOCKED=ERROR decision_ledger_has_no_ids surface=%s' % decisions); sys.exit(1)
if not lay:
    print('S1_FALSELY_BLOCKED=ERROR empty_layer_population surface=%s' % beads); sys.exit(1)
# FALSELY blocked = the two classes that are WORK wearing a hold. AwaitingHumanDecision is a
# REAL hold and is excluded BY A NAMED PREDICATE, not by hand.
false_n = len(cls['TrackerBlocked']) + len(cls['ExecutionOwed'])
print('S1_FALSELY_BLOCKED=%d of %d blocked (population %d)   surface=%s'
      % (false_n, len(blocked), len(lay), beads))
for k in ('DependencyBlocked', 'TrackerBlocked', 'ExecutionOwed',
          'AwaitingHumanDecision', 'Unclassifiable'):
    print('  %-22s %d%s' % (k, len(cls[k]), (' ' + ','.join(cls[k])) if cls[k] else ''))
if cls['Unclassifiable']:
    print('UNCLASSIFIABLE names an HD id absent from the ledger -- ERROR, not an exclusion')
    sys.exit(1)
sys.exit(1 if false_n else 0)
PY
```
**Expect `=0` with every excluded row named.** Measured 2026-09-07: **`0 of 0 blocked (population
138)`**, all five classes empty, `rc=0`. ✅ **PASSING — but read the next paragraph before banking
it.**

> **THE NUMBER REACHED ZERO PARTLY BY FLIPPING TWO ROWS WHOSE CLASSIFICATION WAS NEVER RECORDED, and
> that is the defect this predicate closes.** The trajectory was 92 → 89 → 88 → 2 → 0. The last two
> — `s1-l3-hd0009-lo3g` and `s1-l5-decisions-owed-4tq2` — were the rows `%20` refused to flip,
> because *"flipping them would be falsifying rows to zero a number."* They are now `open`. For one
> of them that is the RIGHT answer for a reason nobody wrote down; for the other it is
> UNDETERMINED:
>
> * **`lo3g` names `HD-0009`, and `HD-0009` HAS A DECISION** — two rows, an empty first row then
>   **385 chars**. So *"halt step until Joshua decides"* was **false**: nobody is awaiting a human.
>   Its true class is **`ExecutionOwed`**, which is WORK, so `open` is correct — and the predicate
>   above now says so instead of leaving it to memory.
> * **`4tq2` names NO HD id at all** (*"L5 decisions_owed with age"* is about the aggregate). Under
>   this predicate it is `TrackerBlocked`, i.e. it counts — which means flipping it was defensible
>   but was never justified by any rule. Had it stayed `blocked`, R3 would read 1, correctly.
>
> **`0 of 0 blocked` is now printed with its denominator** precisely because a criterion that reads
> `0` from an EMPTY population cannot be distinguished from one that read `0` from a healthy one.

**THE DOMAIN HAS FOUR STATES, NOT THREE — measured, and it corrects a generalisation of my own
source.** The three-state framing (`DependencyBlocked` / `TrackerBlocked` /
`AwaitingHumanDecision`) misses the class that actually dominates:

```
docs/decisions.jsonl : 56 rows, 44 distinct HD ids
  ANY row carries a decision : 19   HD-0001..HD-0018, HD-0033
  NO row carries a decision  : 25   HD-0019..HD-0032, HD-0034..HD-0044
  of the 19 decided ids, how many carry an execution receipt
    (execution_status AND executed_at AND actuator_receipt) : 0
```

So **every recorded decision is UNEXECUTED**, and *"awaiting a human"* is true for **25 of 44** ids
— not almost-never. The claim *"all 19 recorded HD decisions carry a real decision"* is **exactly
right about those 19 and does not generalise to the ledger**, whose population is 44. `HD-0008`'s
*"push it"*, recorded 2026-09-02 and never executed, is the archetype and already has a bead
(`hd0008-decision-never-executed-ql7w`). **`ExecutionOwed` is therefore the fourth state, it is the
largest one, and a bead in it is WORK — it must never be excluded as a human hold.**

**FIRES-ON-KNOWN-BAD — five fixtures under `docs/fixtures/`, all executed:**

|fixture|class|counts?|rc|
|---|---|---|---|
|`r3-dependency-blocked`|`DependencyBlocked` — blocked WITH a real edge|no|0|
|`r3-tracker-blocked`|`TrackerBlocked` — no edge, no HD id|**yes**|1|
|`r3-execution-owed`|`ExecutionOwed` — names `HD-9001`, decided in `r3-decisions.jsonl` via an **empty first row** then a decision|**yes**|1|
|`r3-awaiting-human`|`AwaitingHumanDecision` — names `HD-9002`, asked and never answered|no|0|
|`r3-decisions.jsonl`|the fixture ledger; `HD-9001` proves the any-row rule, `HD-9002` the exclusion|—|—|
|`r2-empty.jsonl` as the LEDGER|anti-vacuity arm 1|`ERROR decision_ledger_has_no_ids`|1|
|`r2-empty.jsonl` as the BEADS|anti-vacuity arm 2|`ERROR empty_layer_population`|1|

> **BOTH OF THOSE ARMS WERE DEAD ON THE FIRST RUN, and the fixtures are what showed it.** The
> ledger parser accepted *any* row carrying an `id` as a decision id, so pointing `R3_DECISIONS` at
> a BEAD file produced 1 bogus "decision id", `seen_ids` was non-empty, and the guard **silently
> did not fire**. Fixed by requiring `re.fullmatch(r'HD-\d{4}', hid)`. The same run also caught the
> population disagreeing with R2 — 144 against R2's 138, because R3 was not excluding the six layer
> gates. **Two criteria over one set must never disagree on the denominator**, and only running them
> side by side reveals it.

`r3-execution-owed` is the leg the old predicate could not express: it is `blocked` with no edge and
a human-sounding title, so a hand reading would exclude it as a hold — and it is work.
*Rationale:* `blocked` with an empty graph is never `DependencyBlocked` (`AGENTS.md`). **Proven by
transition, not by reading the field:**
`br update omp-orchestrator-s1-l0-b01-3vro --status in_progress` → `blocked → in_progress`,
accepted, no refusal. **A false block hides work from `br ready` and from every selector.**

**NO-CLAIM.** The any-row rule here is a *transcription* of `crates/decision-ledger`
(`execution.rs`, regression floor `a8e2fc6`), not a call into it. That kernel is the single source
and it also carries the execution half (`nonempty decision without execution_status/executed_at/
actuator_receipt`). It has a `[[bin]]` target and is **NOT on PATH**, and it cannot be installed
today: the lane produces Linux artifacts and local builds are prohibited, which is
`omp-orchestrator-qir1`. **When a darwin artifact lane exists, this runner should call the kernel
instead of restating it** — a transcription can drift from its source, which is the whole defect
class this file tracks.

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
**Expect `=0` and the owner field's number to equal it.** Measured 2026-09-07 by pane 1, per-wave
with the row identities, not just a total:

```
box=S1 rows per wave    wave-1: 12    wave-2: 63
box=S1 BLANK per wave   wave-1:  2    wave-2:  6      TOTAL 8

wave-1   SilverWolf-S1.toml           wave1_sections
wave-1   SilverWolf-S1.toml           sota_cite
wave-2   pane4-jt1i-citations.toml    cite.beads-rust-cli-paths-unprefixed
wave-2   pane4-jt1i-citations.toml    cite.bare-src-main-without-crate
wave-2   pane4-jt1i-citations.toml    cite.mirror-truncated-ellipsis
wave-2   pane4-jt1i-citations.toml    cite.dropped-leading-j-on-jplf-family
wave-2   pane4-jt1i-citations.toml    figure.unlabelled-in-owner-files
wave-2   pane4-qibn-hook-count.toml   box.hook.certified_false_vs_unattempted
```

❌ **FAILING at 8.** The owner field reads `6` because its predicate is wave-2-scoped while this
criterion's own runner globs `waves/*/*.toml` — **all waves**. That mismatch is the entire 8-vs-6
gap and it was the open scope question.

### RULING — wave-1 counts. Pane 1, 2026-09-07.

Three reasons, in order of weight:

1. **A blank resolution is unresolved regardless of which round produced it.** If wave-2 had
   superseded a wave-1 row, that row would carry a resolution *saying so* — every other resolved row
   does, including the fifteen wave-2 rows that read "stays open" or "escalated". **Blank means
   nobody answered it**, which is exactly what this criterion measures.
2. **Excluding wave-1 creates an escape hatch.** A finding could be evaded by having been filed in
   an earlier review round, which inverts the purpose of cross-review.
3. **Wave-1 is not a stale dumping ground — it is proportionally MORE resolved.** 10 of 12 answered
   (83%) against wave-2's 57 of 63 (90%). Comparable diligence, so there is no empirical basis for
   treating one round as authoritative and the other as noise.

**Action:** widen the owner field's predicate to all waves so it reads `8`, then work the rows. `%7`
landed the derived predicate at `7b4ac63`; it needs the glob widened, not a new mechanism.
**A hardcoded `0` is not a measurement** — S2–S9 owner files each carry a literal
`open_disagreements=0` with no predicate at all (`S2:120 · S3:171 · S4:178 · S5a:135 · S5b:140 ·
S6a:137 · S6b:132 · S6c:137 · S7:165 · S8:175 · S9:132`), which is the never-fires-so-always-green
class sitting under this gate.

### AMENDMENT — I looked for evidence against this ruling, found some, then refuted it

Recorded as a sequence because each step was driven by a new measurement and the last one is the
answer. **Three positions in one pass; the third is grounded in the resolution VALUES rather than a
counter, which is why it is the durable one.**

**Step 1 — the wave-2 scoping was deliberate, not a defect.** `boxes/S1.toml:334` reads
`open_disagreements = 6 # DERIVED for wave=2: …`. `%7` scoped it on purpose and said so at the
field. My framing implied an oversight; it was a choice.

**Step 2 — an in-repo basis for excluding wave-1 exists, and I had wrongly dismissed it.**
`boxes/S1.toml:336`: `rejections = 25 # … wave-1 = 0 of 12 (a SURVEY, not a review) …`. A scout
reported this substance citing `CONTRACT.md:56-57`; I checked that citation, found it did not
support the claim, and filed the claim as **unsupported**. The citation was wrong; the substance was
real and lives here. **A wrong address does not make a claim false** — I should have searched for the
assertion before ruling against it.

**Step 3 — and the survey characterization is REFUTED by the data it rests on.** Distinct resolution
values for `box = "S1"`, measured 2026-09-07:

```
wave-1 (12 rows)                      wave-2 (63 rows)
  accepted-and-landed      6            resolved-at-7f9195a    29
  accepted-and-amended     2            accepted               15
  accepted                 2            escalated               2
  <BLANK>                  2            <BLANK>                 6
                                        L1 / The / NTM          3  <- malformed, prose leakage
```

**Wave-1's resolutions are `accepted-and-LANDED`** — the disagreement was accepted *and the fix
shipped*. That is a **stronger** disposition than wave-2's dominant `resolved-at-<sha>` and nothing
like "acknowledged." **`rejections = 0 of 12` is true and non-discriminating: wave-1 had zero
rejections because everything was ACCEPTED, not because nothing was reviewed.** My own first
predicate confirmed the non-discrimination — a `REJECT`-prefix scan returns **{} for BOTH waves**,
so the asymmetry the survey claim depends on does not exist under any predicate I could construct.
(`:336`'s own `25 of 50` uses a third denominator and is flagged `WORKTREE ONLY -- and that is a
defect` two lines above.)

**RULING STANDS AT 8, now on three intact reasons.** Reason 3 is reinstated with better evidence
than I first gave it: wave-1 was not a survey, it was a review whose findings **landed**.

**AND `S1.toml:336` CARRIES A FALSE PARENTHETICAL.** "(a SURVEY, not a review)" is refuted above. It
matters because, if ratified, it would drop two genuinely unresolved rows on the strength of a `#`
comment that no command produces — in a field whose two neighbours are already flagged worktree-only
defects. Correcting it belongs to `S1.toml`'s owner, not to this file.

**Three malformed wave-2 rows surfaced en route** (`L1`, `The`, `NTM`): multi-line TOML prose
leaking into the `resolution` capture. Same parser defect measured earlier this session, still live,
and it means **any single-line resolution predicate slightly understates the real count.**

### THE 8 ROWS ARE 3 WORK ITEMS, AND 6 OF THEM ARE ORPHANED

**5 of the 8 come from a single file** and are one coherent batch: `pane4-jt1i-citations.toml`'s four
`cite.*` rows plus `figure.unlabelled-in-owner-files` are all citation-hygiene findings. Treating
them as five independent blockers overstates the remaining work by ~2.5×.

**But `pane4-*` files belong to pane 4, which left the fleet** (credits exhausted; roster since
changed). So **6 of the 8 rows — the 5 `jt1i` plus the 1 `qibn` — have no live owner**, and the
reviewer who filed them cannot answer them. Per this repo's rule that availability and validity are
different facts, those findings **remain valid and must be adopted by a live pane, not voided**.
Only SilverWolf's 2 have a reviewer who could in principle still respond.

**Real shape of R4:** one batch of 5 citation rows needing an adopter, 1 hook-count row needing an
adopter, and 2 SilverWolf rows. Three items, not eight.

### R4 CLEARED 2026-09-07 — and 3 of the 8 rows were MY INSTRUMENT, not their subject

**Verified independently by pane 1 before banking it:** 43 wave files scanned, **75** `box = "S1"`
rows, **0** with a blank resolution. `boxes/S1.toml:334` now reads
`open_disagreements = 0  # DERIVED, ALL WAVES, widened from wave=2 by %19` — a predicate, not a
hardcoded integer. Landed `f701dfc`; `%19` answered the rows and deliberately did **not** close it.

**THE DURABLE FINDING IS THE SELF-REFERENTIAL CORPUS, and it fired twice in five rows:**

> **A citation-hygiene scan over a corpus that CONTAINS its own defect reports finds its own
> specimens.** The fix is to strip quoted specimens before matching — **never to edit the documents
> that record the rule.**

- `cite.mirror-truncated-ellipsis` → **REFUTED.** The only `beads_rust` + ellipsis match in
  `CONTRACT.md` is inside the sentence that **refuses** the form: *"a scout labelled ack-spine and
  no-shell-gate as `mirror:beads_rust/...` — fabricated; refused"*. A quoted specimen inside the
  rule against it.
- `cite.dropped-leading-j` → **CLEARED.** The 2 survivors live in the **PX-D5 row that reports this
  very defect.** Positive control: `plf.7.2` → **0** rows in `issues.jsonl`, `jplf.7.2` → **1**. No
  live citation carries the dropped `j`.
- `wave1_sections` (blocker) → **REFUTED, structurally.** All four sections were present at the
  review-date tree `e5ab541`: 6 measurements, 10 gaps, 1 agreement, 1 diagram. The reported
  `0,0,0,0` reproduces **only** under a bare `[[<key>]]` matcher, which drops the `box.` prefix
  **and cannot see single-bracket tables at all** — `agreement` and `diagram` are `[box.x]`, not
  `[[box.x]]`. **A structurally-guaranteed zero**, and it was already so at the review date, so it
  is not drift.

**The row that UNDERSTATED itself, and it is the never-fires class:** `figure.unlabelled-in-owner-files`
named `boxes/S1.toml`, which **carries a predicate and is not the offender.** The real population is
**11 sibling owner files (S2–S9) plus `CONTRACT.md`**, each with a bare `open_disagreements = 0` —
**no predicate, no TREE/WORKTREE label.** Re-scoped to those 12 for their owners.

**One residual pane 1 owns and did not delegate:** `boxes/S1.toml`'s `rejections` comment still
carries *"wave-1 = 0 of 12 (a SURVEY, not a review)"* — the exact claim refuted at `166c078`. **The
refutation landed; the prose defending it did not.** `%19` found the same shape in `qibn` (a header
comment still arguing `certified = false` above eight rows now reading `UNATTEMPTED`).
**Prose defending a value the file no longer holds, invisible to every predicate.**

### R5 — every S1 layer gate NAMES its known-bad leg

**Naming is the whole criterion. It does not measure firing** — that is R9, added below because I
wrongly folded it in here. Retracted at `ec7d93d`; see the retraction note under the scorecard.

```bash
# Strip any SUBJECT: echo, then require the hit INSIDE A NUMBERED ITEM, case-insensitively.
for g in l0-jtgw l1-fnv8 l2-j5m9 l3-z8hz l4-hs15 l5-w44h djn8; do
  br --lock-timeout 60000 show "omp-orchestrator-gate-s1-$g" --json | python3 -c "
import json,sys,re
a=(json.load(sys.stdin)[0].get('acceptance_criteria') or '')
body=chr(10).join(l for l in a.split(chr(10)) if not l.strip().upper().startswith('SUBJECT:'))
n=len([l for l in body.split(chr(10)) if re.match(r'\s*\d+[.)]',l) and 'known-bad' in l.lower()])
print('$g', 'PASS' if n else 'FAIL', 'numbered-items-naming-known-bad=%d' % n)"
done
```
**Expect PASS on all seven.** Measured 2026-09-07 23:0xZ:

```
gate       lower   UPPER   numbered   SUBJECT:-echo
l0-jtgw    1       1       1          1
l1-fnv8    1       1       1          1
l2-j5m9    1       1       1          1
l3-z8hz    1       1       1          1
l4-hs15    1       1       1          1
l5-w44h    1       1       1          1
djn8       0       1       1          0
```

✅ **PASS — 7 of 7.** Every gate carries a numbered KNOWN-BAD item, and `%20` reports a KNOWN-GOOD
leg and an ANTI-VACUITY clause alongside it (`8hq3`, P0). Filed by `%20`; **I am ineligible to
grade it.**

**MY FIRST RUNNER WAS WRONG THREE WAYS AND `%20` FOUND ALL THREE:**

1. **TITLE ECHO.** Each layer gate's `subject-echo = 1` — the lowercase `known-bad` hit lives in a
   restated `SUBJECT:` header **inside the acceptance field**. My caveat was "the title is not the
   acceptance field"; the sharper fact is that **the title is pasted INTO it**, so a naive grep
   cannot distinguish a requirement from an echo. Strip `SUBJECT:` and demand a numbered item.
2. **CASE.** The requirement is spelled `KNOWN-BAD` uppercase. `djn8` has `lower=0, UPPER=1`, so a
   lowercase-literal grep returns a **false negative** on it outright.
3. **DEFINITION DRIFT — mine, and the worst of the three.** I renamed this criterion to "names a
   known-bad leg AND has fired" and reported FAIL. **Naming was always the predicate.** Widening a
   criterion mid-measurement to fail it is moving the goalposts, and `%20` asked directly that R5
   not be flipped to a firing claim on its evidence. Honoured.

**WHERE `%20` IS WRONG, measured:** it attributes `djn8`'s missing message assertion to the same
case artifact and reports item 4 as *"KNOWN-BAD LEG asserting the MESSAGE"*. `djn8`'s acceptance
contains **`MESSAGE` 0 times, `message` 0 times, `asserting` 0 times**; its known-bad is **item 1**,
verbatim *"KNOWN-BAD (premise) — MUST BE EXECUTED FIRST: run the live census `br dep tree GATE-S1
--depth 1`…"*. My `no` was correct. **That strengthens `%20`'s own genuine-gap finding rather than
weakening it**, because `djn8` also contains `exit code` **twice** and `REACHABLE`/`reachable
trigger` **zero** times:

> **The STAGE gate asserts on an EXIT CODE with no message clause and no reachable-trigger clause.**
> Per AGENTS.md gate rule 7 that is the exact known-bad-leg defect: `101` is cargo's generic
> failure and an unrelated workspace-loading error produced an identical `101` in this repo, so a
> leg keyed on `rc != 0` goes green on unrelated breakage. `8hq3` item 7 closes the trigger half;
> the message half needs adding to it.

### R9 — has any S1 layer gate ever FIRED (new row; my R5 measurement, re-homed)

```bash
grep -ci 's1-l' .github/workflows/gate.yml    # expect >0 if any layer gate runs in CI
crontab -l 2>/dev/null | grep -ci 's1-l'      # expect >0 if any layer gate runs on a timer
for g in l0-jtgw l1-fnv8 l2-j5m9 l3-z8hz l4-hs15 l5-w44h djn8; do
  br --lock-timeout 60000 show "omp-orchestrator-gate-s1-$g" --json | python3 -c \
    "import json,sys;print('$g', json.load(sys.stdin)[0].get('status'))"
done
```
Measured 2026-09-07 by `%20`: `gate.yml` **12 jobs, 0 naming `s1-l`**; `crontab` **0**; all seven
gates `status=open`, so not one has ever completed.

> **The job count was 14 here and is 15 in `fsu7`; both are counting artifacts and the answer is
> 12.** `.github/workflows/` holds **one** file. It has **15 keys at two-space indent**, but three
> of them — `push`, `pull_request`, `workflow_dispatch` — sit under `on:` and are TRIGGERS, not
> jobs. Counting keys after the `jobs:` anchor gives **12**: `no-shell-gate`,
> `head-compiles-as-committed`, `path-literal-guard`, `grader-attribution-gate`,
> `undrained-pipe-lint`, `kernel-bypass-gate`, `state-wildcard-lint`, `installer`,
> `omp-inventory-map`, `pre-delete-citation-check`, `commit-build-fence`, `porting-gate`.
> **An indentation-keyed count cannot tell a job from a trigger** — the same class as a `[[bin]]`
> grep that reports `tick-monitor` as 0 while it is on PATH. The `0 naming s1-l` half is unaffected.

**WIRING FEASIBILITY, measured — R9 CANNOT BE FULLY WIRED TODAY, and wiring what exists would be a
false green.** `INERT → wire it` is the right remedy, and the inventory says what is wireable:

|layer|named per-layer target that exists today|
|---|---|
|L0|`crates/installer/tests/l0_install.rs`|
|L1|`crates/ompo-doctor/tests/l1_doctor.rs`|
|L2|`crates/ompo-start/tests/l2_ecosystem.rs`|
|L3|`crates/ompo-start/tests/l3_step_parity.rs`|
|L4|`crates/ompo-start/tests/l4_pack.rs`, `crates/ompo-start/tests/l4_spawn.rs`|
|L5|`crates/ompo-start/tests/l5_foundation.rs`|
Plus `crates/s1-coverage/tests/contract.rs`, which is the coverage matrix rather than a layer leg.
**All six layers now have a named target; L1 and L2 were the planning halves `%7` and `%8` held.**
The new targets are real integration targets over `ompo-doctor` and `ompo-start`. **No layer gate's
known-bad leg exists as code at all**: the legs are specified in the gate beads' acceptance text
(R5 = 7/7 on NAMING), not implemented.

Attaching the six named targets to an entry point must happen as one six-leg gate, not six separate
workflow keys. Until that single entry point exists and fires, R9 remains INERT rather than reading
the target inventory as gate coverage.
**WHERE TO ATTACH, and it is not a workflow key.** `omp-orchestrator-fsu7` establishes that CI is
11 gate crates fanned out by YAML with no single entry point, and `%19` measured that **Actions is
STRICT** — n=60, and 49 duplicate-key runs started **ZERO** jobs. Seven new workflow keys is seven
new single-colon failure modes that fail SILENT-ZERO. The S1 layer gates hang off the single Rust
entry point `fsu7` describes, or off one `cargo test` target that names all six — **one key, six
legs**, so a YAML defect cannot zero five gates while leaving one green.

⚠ **UNMEASURED as a firing result — classification INERT.** Per gate rule 4a the four
classifications carry different remedies: **ABSENT** → write it, **INERT** → wire it, **UNRUN** →
run it, **FIRED** → measure the result. The gates exist, are wired as bead dependencies, appear in
`docs/contracts/` (`s1_l0_install.md:159`, `s1_l3_walkthrough.md:148`, `s1_l4_liveness.md:156`,
`s1_l5_portal.md:150`), and **nothing invokes any of them. INERT means WIRE IT.**

**POSITIVE CONTROL, mandatory:** the pre-commit multi-gate **does** fire and the same method sees it
(`gate.yml:449-450` invoking `pre-delete-citation-check`; observed firing three times today).
**`%20` measured those firings and refused to count them toward this row, then applied the same
refusal to its own R5 result.** That is the discipline to copy: a different gate family firing is
not evidence about this one.

### R6 — the box's diagram receipt matches a fresh validator run
```bash
frankenmermaid validate docs/plan/flow/diagrams/S1.mmd --fail-on warning
grep -E 'validate_exit|nodes|edges' docs/plan/flow/boxes/S1.toml
```
**Expect exit 0 and the checked-in `nodes`/`edges` to equal the run.** Measured: **PASSING** after
`f6bbc1c` corrected a stale `52/60` to a re-run `53/64`, and added `validate_command`. ✅
*Rationale:* a `validate_exit = 0` recorded from an earlier revision is a fooled certificate.

### R7 — S0 is closed

**TWO oracles, because the first runner was broken and could not report anything else.**

**Oracle A — the epic's own refusal.** `br` refuses to close an epic over open children and NAMES
the count, independent of any query we write:

```bash
br --lock-timeout 45000 close omp-orchestrator-s0-asupersync-misapplication-audit-kvsq \
  --reason PROBE --actor r7-probe 2>&1 | head -1
```

**Expect no `open children` warning once S0 is closed.** Measured 2026-09-07:
**`epic has 2/13 open children`.** ❌ **FAILING — for a bounded, named reason; see below.**

**Oracle B — the child set, derived with the key that surface actually uses.**

```bash
python3 - <<'PY'
import json
EPIC='omp-orchestrator-s0-asupersync-misapplication-audit-kvsq'
rows=[json.loads(l) for l in open('.beads/issues.jsonl') if l.strip().startswith('{')]
by={}
for r in rows:
    if r.get('id'): by[r['id']]=r          # jsonl is an append-log; later lines win
# IN-edges: children point AT the epic. `br dep list` shows OUT-edges only and CANNOT see these.
kids=[(i,r) for i,r in by.items()
      for d in (r.get('dependencies') or [])
      if d.get('depends_on_id')==EPIC and d.get('type')=='parent-child']
assert kids, 'ANTI-VACUITY: zero children resolved -- the runner is broken, not S0 finished'
open_=[(i,r) for i,r in kids if r.get('status') not in ('closed','tombstone')]
print('EPIC=%s  CHILDREN=%d  NON_TERMINAL=%d'
      % (by.get(EPIC,{}).get('status'), len(kids), len(open_)))
for i,r in sorted(open_):
    print('  ', r.get('status'), 'P%s'%r.get('priority'), i.replace('omp-orchestrator-',''))
PY
```

**Expect `EPIC=closed` and `NON_TERMINAL=0`.** Measured 2026-09-07: **13 children, 2 non-terminal.**

> ⛔ **THE PREVIOUS RUNNER NEVER CONSULTED THE GRAPH. Finding: `omp-orchestrator-pt5w` (P0).**
> It read `{d['id'] for d in (by.get(EPIC,{}).get('dependencies') or []) if d.get('id')}` — two
> defects, the second masking the first. **Direction:** it read the epic's OUT-edges, while
> `parent-child` ownership is an IN-edge. **Cross-surface consumption:** `d.get('id')` is *correct*
> for `br show --json`, whose dependency dicts are `dependency_type, id, priority, status, title`,
> and it was pointed at the **jsonl**, whose dicts are
> `created_at, created_by, depends_on_id, issue_id, metadata, thread_id, type`. **The two key sets
> share ZERO names**, so the comprehension evaluated to `set()` **unconditionally** — verified with
> three real OUT-edges present. `br list --json` is a third shape with **no `dependencies` key at
> all**, only `dependency_count`. Fixing the direction alone would have left the clause empty.
> **The criterion has always rested on the `s0-` name prefix while reading as a graph query.**
> `%20` found it; the bounded blast radius is in `pt5w`, and the in-tree fix pattern is
> `bead-availability/src/inversion.rs:426-436` — a typed error naming the key it wanted — with its
> fires-on-known-bad leg at `:636-644`.
>
> **The `assert kids` line is not decoration.** A runner resolving zero children and one resolving
> the right set are indistinguishable when every input is empty, which is exactly how this survived.

> ⛔ **SCOPE CORRECTED TWICE.** First: `jplf.1.1` and `jplf.7.2` were counted as S0 children and are
> not — they source from `docs/plan/05-actions.md:L253-L262` and
> `docs/plan/07-installability.md:L113-L116`. Second, 2026-09-07: **four children were parented onto
> the epic that are not asupersync-audit work at all** — `wo0c` (blocked-agent notification), `f4xl`
> (contabo-3 DNS), `maco` (`decisions.jsonl` duplicate ids), `nxc9` (M2 grading lane). Their
> `parent-child` edges were removed and the gate moved **6/17 → 2/13**. **The audit was never what
> was blocking S0.**
>
> **Removing those edges also surfaced two hidden beads:** `maco` and `nxc9` went `br ready`
> **False → True**. **Being parented under an open epic hides an open bead from the queue while
> leaving it claimable** — `%20`'s phrase, measured independently on `jix1`, is *"claimable but no
> longer offered."* A reproducible instance of `kt0m`'s 194.

**The predicate is the AUDIT SLICES, never the `phase-0` label.** Measured: 23 non-terminal
`phase-0` rows = **3 slices + 20 findings the audits produced**. `w21v` → `qfw0`/`ciab`; `zaxp` →
`3kcl`; pane-1 work → `47g0`/`fsu7`. **Every successful audit ADDS phase-0 rows**, so a criterion
keyed on "phase-0 is empty" is unsatisfiable by construction — the never-fires class, same as
docs-staleness re-staling in 25 minutes on a wave.

**S0's audit is DONE.** All five `s0-audit-*` slices are terminal — `w21v`, `zaxp`, `yt9l`, `eoqd`,
`xv5r` — and `jix1` closed `APPROVED` as a proof-of-block after `%19` proved `messaging-fabric` is
coupled to `test-internals` by a production call site (`consumer.rs:1299-1300`; the feature gate is
at `messaging/mod.rs:81-82`, **not** the crate root). **The epic is gated on one dependency and one
human:**

```
f3g5  P0  open  block every non-arc bead behind S0   <- blocked by kt0m
lppp  P2  open  darwin process/reap semantics        <- JOSHUA-DECISION
```

*Rationale:* S0 is the asupersync-correctness floor S1 stands on. Building S1 layers on an unproven
cancellation contract means every layer inherits an unmeasured defect. **`lppp` is titled `P0
JOSHUA-DECISION` and filed at P2** — it gates S0 closure from a priority where no selector will
offer it, which is its own defect.

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
|R2 layer beads wired to their gate|✅ **PASS** — **0 of 138, STRANGLED=0**, snapshot 2026-09-07 (`rc=0`). **Was ⚠ WEAK** until `2fxd` made the predicate require *own-layer-gate* linkage, made the transpose a typed `STRANGLED` refusal, added an anti-vacuity arm, and shipped four executed fixtures. Retired predicate printed beside it on every run. History: published **FAIL 144/144** (wrong at publication — the edges predated this file by 4 days) and its first "correction" (`id`/`dependency_type`, `br show`'s keys, not the JSONL's) **also wrong**|
|R3 no false `blocked`|✅ **PASS — `0 of 0 blocked (population 138)`**, `rc=0`, all five classes empty, denominator now agreeing with R2. **Was ⚠ NEARLY (2).** Trajectory 92 → 89 → 88 → 2 → 0, and **the last two were flipped without their class being recorded** — `lo3g` correctly (`HD-0009` HAS a 385-char decision, so its "awaiting Joshua" was false; true class `ExecutionOwed`, which is WORK), `4tq2` undetermined (names no HD id → `TrackerBlocked`, so it counts). The predicate now records the reason instead of leaving it to memory, and prints the blocked-denominator so a `0` from an EMPTY population is distinguishable. **FOUR states, not three:** the missing one is `ExecutionOwed` and it is the largest — **19 of 44 HD ids carry a decision and ZERO of those 19 carry an execution receipt**|
|R4 disagreements resolved, derived count|✅ **PASS — 0 of 75** across **43 wave files** (`f701dfc`, `%19`). Owner field widened to `DERIVED, ALL WAVES`. **3 of the original 8 were INSTRUMENT defects**, not subject defects|
|R5 gates NAME a known-bad leg|✅ **PASS — 7 of 7** (strict: `SUBJECT:` stripped, numbered item required). `8hq3` P0 filed by `%20`|
|R9 has any gate ever FIRED|✅ **PASS — SCOPED TO THE TEST HALF, and a gate FIRED RED on lane.** All seven layer targets ran individually on contabo, `RCH_REQUIRE_REMOTE=1`, zero bypass rows: **`l1_doctor` 3/0, `l2_ecosystem` 2/0, `l3_step_parity` 2/0, `l4_pack` 2/0, `l4_spawn` 2/0, `l5_foundation` 1/0 GREEN; `l0_install` FIRED-RED exit=101, 25 passed / 1 failed** on `real_atomic_install_publishes_complete_binary`. **A gate has fired — that is the criterion, and it fired in the RED direction on real infrastructure rather than on a fixture.** Wired at `fsu7`'s single Rust entry point (`gate.yml` 12 jobs → **1**, `3edb872`), which enumerates **286 invocations across 88 crates** with zero ledger drift; the six layers are `--test` targets so `run_crate` executes them. **L0's RED IS ENVIRONMENTAL AND PROVEN, NOT INFERRED:** lane `IdentityMismatch { head: "unavailable", build_id: "unavailable~/" }` ← `identity_head()` → `build_id()` needs git, and `~/.config/rch/config.toml` excludes `".git/"` — verified, line 29. Local positive control **26/0** under a labelled `RCH_CARGO_WRAPPER_BYPASS=1`, cited **only** as the local arm of a two-arm comparison, never as a lane figure. **So L0 is UNMEASURABLE-ON-LANE and GREEN-LOCALLY, and both halves are required to say that** — the lane figure alone reads as a product defect, the local figure alone proves nothing about the lane. **Fifth instance of the transfer class** (`br` absent, `.beads` excluded-but-tracked, `RCH-E301`, mirror ENOENT, now `.git/`). **NOT CLAIMED, three residuals:** (1) **the RUN half has never executed** — `etyur` (P0): `derive_checks` lives at `gate-runner/src/lib.rs:559` with **0** references in `main.rs`, both `Command::new` sites are `cargo`, so 13 crates' declared `checks` are parsed, graded `Covered{checks:true}`, and never run; (2) the full **286**-invocation `--run` is untouched — the scoped run decides this row, it does not discharge `fsu7` item 6; (3) per-crate `targets=5`/`targets=9` are **counts, not names**, and attribution in that path is structurally impossible (`bounded_output` returns stdout/stderr as separate buffers, so every `test result:` precedes every `Running` header), so *"the declared targets are the ones that ran"* stays **UNMEASURED**. **Two of `%19`'s own instruments were caught by the negative-control rule first, one already committed as a "MEASURED LANE FACT" and retracted in-source:** the `Running` header probe searched the adjacent literal `Running unittests` while an **ANSI reset sits between the two words** — the very log cited as evidence carries **3** `Running` lines. And `--only` printed **87 spurious `LEDGER_DRIFT` rows** by comparing the 88-row ledger against the *filtered* roster, with drift ranked above gate failures, which would have made the flag useless for the exact decision it exists to serve (`build_report_scoped`, fixed, control re-run). **Mutation-verified:** `"unknown".to_owned()` → RED, ANSI-strip removal → RED, restore `eef5c7e852e53a23` byte-identical, 37/37. The retired `"unknown"` form survives **only** as two `!any(|f| f == "unknown")` assertions that it is absent — an in-tree negative control, comment-stripped count 2, so the reason the retired probe returned zero cannot be lost again|
|R6 diagram receipt matches a fresh run|✅ **PASS** — 53/64, exit 0|
|R7 S0 closed|✅ **PASS — `kvsq` closed 2026-09-07, `close_reason` starts `DONE`.** All nine S0 rows terminal: five `s0-audit-*` slices (`w21v` `zaxp` `yt9l` `eoqd` `xv5r`), `jix1` (`APPROVED` as a proof-of-block), `kt0m`, `lppp` (`APPROVED` by `%19`), and `f3g5` (`MUTATION-VERIFIED` by `%8` on items 1–8). **Every child closed by a pane that did not author it** — `f3g5` got the strongest posture available: `%7` graded items 1–6, `%8` independently re-executed 7–8 on contabo-4, disjoint scopes, neither the author. **`f3g5` item 9 is ruled to `gb28`** as UNRUN-pending-restart, not deferred-as-impossible — I made the impossibility error on item 1 of that same bead and `%7` refuted it. **THE PREDICATE IS THE NINE S0 ROWS, NOT THE `phase-0` LABEL:** 23 non-terminal `phase-0` rows were measured as **3 slices + 20 findings the audits produced**, so a criterion keyed on "the label is empty" is unsatisfiable by construction — the never-fires class. **DISCLOSED: the epic was first closed BY ACCIDENT** by my own gate-probe (`br close --reason PROBE`, used ~20× as a read-only query; every prior run returned a refusal I read as data, and the run after the last blocker cleared performed the write). Corrected by reopen → re-close with a real reason and actor; the `PROBE` reason and the fake actor are left visible in a disclosure comment rather than erased. **Second finding from that accident: the close-reason policy is PROSE, not a mechanism** — the literal string `PROBE` was ACCEPTED where this repo documents a non-conforming reason as "refused by policy"|
|R8 instruments not stale/self-referential|✅ **PASS — NARROWED BY AUTHORITY, and the narrowing is the whole story.** Surviving half ✅: the self-referential runner `2a9df28`, on-lane **10/0**. **Two halves were removed from the criterion by decisions, not by measurement, and each is named so the smaller coverage is visible:** (1) the **fh half** — root-caused to a human TCC grant plus a franken-harvest policy fix — **DROPPED BY JOSHUA**, verbatim *"your job isn't fixing fh"*; (2) **tree-cleanliness DROPPED** after `%19`'s `lsof +D` probe returned **0 for the live checkout's own `crates/` too**, so it had **no discriminating power** — a predicate that cannot separate the healthy case from the broken one is not a weak signal, it is no signal. **This is the R5 sin in the opposite direction** (narrowing to pass rather than widening to fail), so the residual is stated rather than implied: **fh's staleness is UNMEASURED and remains so**, and no instrument in this repo currently watches it. A reader must not read this PASS as "all instruments verified"|
|**R10 the gate layer RAN**|✅ **PASS 2026-09-08 — BOTH HALVES CLOSED; the FAIL history below is kept because it says why R10 exists.** ORIGINALLY ADDED 2026-09-07 BECAUSE THE OTHER NINE CANNOT SEE THIS.** `R1`–`R8` are **all static**: acceptance text present, edges wired, no false `blocked` strings, disagreements resolved, gates *name* a known-bad leg, a diagram receipt, S0 closed, instruments not self-referential. **Not one requires the system to run.** `R9` is the only execution row and it passed **scoped to the test half**. So *"nine of nine"* and *"no evidence the system runs"* are **consistent readings of the same board** — and per rule `8i`, **a criterion set that cannot distinguish "runs" from "does not run" cannot answer whether S1 is done.** **All four lanes said so independently, unprompted, in answer to *"what would you NOT claim S1 delivered"*:** `%20` — *"every instrument I built is static … S1 delivered provable structure. It delivered no evidence that the system RUNS"*; `%7` — *"S1 delivered provable structure and local evidence boundaries, not a complete runtime certificate"*; `%19` — *"286 invocations, first execution ever is in flight … and it delivered no evidence the instruments are sound as a family, only that a mutated leg is"*; `%8` — *"no runtime proof … RCH proves the test process ran remotely, not that the unattended orchestrator executes the system."* **THE PREDICATE, in two parts, both falsifiable and both in flight:** (a) **`gate-runner` banks a durable verdict for every one of the 88 roster crates** — `fsu7` item 6, via a SINGLE `--run` invocation with `9gta3`'s streaming so a kill at crate 80 does not lose the record that 79 passed. **CHUNKING WAS WITHDRAWN 2026-09-07 by its own proposer** after the measurement refuted the premise: one invocation completed in 1289 s (21m29s) and banked all 88 rows, so 88 `--only` calls would cost 88 transfer setups and lose the shared workspace build to buy protection against a kill that does not happen; and (b) **at least one DECLARED check actually executes** — `etyur`, where `derive_checks`, `expand` and `subsumption` are measured **test-only with zero production consumers**, so 13 crates' declarations are graded `Covered{checks:true}` by a chain nothing calls. **SCOPE, stated so this row cannot be over-read:** R10 is about **the gate layer** running, not the orchestration loop. The dispatcher is out of scope by Joshua's standing ruling and `gb28` holds `f3g5` item 9 as UNRUN-pending-restart; **nobody is to act on it.** **WHY THIS IS NOT MOVING THE GOALPOSTS:** the nine criteria were satisfiable without execution, which is a defect in the criteria and not an achievement to preserve. `9gta3` measured the consequence — `88 x 600s = 14.7h` worst case against a **6h** GitHub Actions ceiling, **no aggregate deadline** among six `*_DEADLINE` constants, and `report.render()` after the loop — so **the entry point `fsu7` built to BE CI's gate cannot complete in CI and emits nothing when killed.** A row that passes while that is true is measuring the wrong thing|

**ALL TEN CRITERIA NOW PASS.** Nine were static; the tenth — did the gate layer actually RUN — was answered on 2026-09-08 by an execution, not a reading. **What that execution REVEALED is new work and is named below; it is not folded into R10’s verdict.**

### R10 CLOSED 2026-09-08 — BOTH HALVES. The gate layer RAN.

```
part (b)  etyur   CLOSED   DONE ALREADY-FIXED/PREMISE-FALSE   graded by %19, non-author
part (a)  fsu7    CLOSED   DONE                               executed by %7
```

**Part (a)'s evidence, re-verified by pane 1 rather than accepted from the report:**

```
DENOMINATOR   docs/gate-roster.txt  88 non-comment rows
              cargo metadata        88 packages          <- TWO INDEPENDENT DERIVATIONS AGREE
BUILD         Contabo, `--config build.target=`, NEVER --target · exit=0
ARTIFACT      `file` checked BEFORE execution: Mach-O 64-bit executable arm64
SINGLE RUN    2857 s (00:47:37), no --only chunking
BANK          .../pane7/fsu7-20260908T170622Z/gate-runner-bank.rows   8,029 B
              sha256 c5fcf89822debe7135a7f48664a9fe5b38af57af12eeb4e734f87cc4767fe31e
              pane 1 recomputed the digest: BYTE-IDENTICAL
              88 unique crate rows (108 total incl. check-level) · set_difference=0
              PASS 59 · FAIL 17 · UNMEASURABLE 12   <- pane 1's independent recount MATCHES
ANTI-VACUITY  a guaranteed-absent --only scope -> GATE_RUNNER_EMPTY_ROSTER, exit=3,
              AND NO BANK WAS CREATED. An empty scan produced neither a pass nor a file.
```

**Why 17 FAIL and 12 UNMEASURABLE do NOT make this a FAIL.** R10 asks whether the gate layer
**RAN**, and it was added precisely because the nine static criteria could not see execution.
Seventeen failures are **the output of a working instrument**, not evidence it is broken — an
all-green run over 88 crates would have been indistinguishable from a vacuous one, which is the
failure gate rule 4 exists to prevent. `%7` declined the stronger claim in its own words: the
close *"does not claim all gates pass, does not treat UNMEASURABLE as failure or success, does
not prove R10(b), and does not authorize installation or deployment."*

**And check-level outcomes were kept separate rather than laundered:** gate-runner reported
`CHECKS executed=20 failed=19` alongside the per-crate rows. Collapsing those into crate
verdicts would have inflated the proof class — a crate whose single check failed and a crate
that was never measured are different facts, and the bank keeps them apart.

**THE RESIDUAL, stated loudly because a passing R10 must not hide it — AND THE FIGURE IS DISPUTED,
see `pd5ua`.** The banked local run reports **17 crates FAIL and 12 UNMEASURABLE** against the
declared roster. That is a **new, measured, per-crate work list** — the first this repo has ever
had — and it is downstream of S1, not part of it. It must not be read as S1 debt, and S1 must not
be read as clean because it closed.

**⛔ DO NOT ACT ON THAT FIGURE WITHOUT READING `pd5ua` FIRST.** Measured 2026-09-08: CI's latest
`GATE_RUNNER` line over the **same 88-crate roster** disagrees with the bank:

```
CI run 34171417882   crates=88  pass=70  fail=16  unmeasurable=2      sum 88
fsu7's banked rows   crates=88  PASS 59  FAIL 17  UNMEASURABLE 12     sum 88
delta                pass +11 · fail -1 · UNMEASURABLE -10
```

**`UNMEASURABLE` maps to a typed `MISSING_EXECUTABLE` reason, so CI measured ten crates the local
run could not — CI is the STRICTLY MORE COMPLETE reading, and this document was citing the less
complete one as the work list.** Both figures are kept here on purpose: the local numbers are what
R10 was closed against, and deleting them would make R10's close unverifiable. `pd5ua` owns
picking the authoritative reading and enumerating the ten.

**This does NOT retract R10.** R10 asked whether the gate layer RAN, and it ran both times. What
was wrong is downstream: a work list is only useful if it is the best available one.


**`etyur` did not close on a source reading — it closed on an EXECUTION.** `%19` ran the remote
`gate-runner --run` and observed per-crate rows, which is the distinction the criterion exists to
make:

```
PASS          crate=ack-spine          targets=10
PASS          crate=ack-stage          targets=7
FAIL          crate=agent-mail-native
UNMEASURABLE  crate=finding            reason=MISSING_EXECUTABLE
... additional PASS/FAIL/UNMEASURABLE rows followed
```

**A `FAIL` and an `UNMEASURABLE` row in that output are what make the grade admissible.** An
all-green run would be indistinguishable from a vacuous one; a run that reports
`MISSING_EXECUTABLE` as its own third outcome class is demonstrably reading real subjects. The
source readback confirms `main.rs:231-250` iterates the checks and calls `run_declared_check`, and
`main.rs:287-290` emits `executed` / `failed` / `declared_crates` — so *declared* and *executed*
are separately reported, which was the premise the bead disputed.

**`%19`'s NO-CLAIM, preserved:** the run is red or unmeasurable for several checks. **This grade
proves the run half EXECUTES and is NON-VACUOUS — it does not prove every declared check passes.**
Those are different facts and only the first was ever R10 part (b).

**What part (a) still owes:** a banked verdict for **all 88 roster crates** with PASS / FAIL /
**UNMEASURABLE** counted separately, the roster denominator re-derived rather than cited, and the
banking proven durable across an interrupted run (`lib.rs:442`). Chunking is **withdrawn by its own
proposer** — one `--run` banked all 88 in **1289 s** — and must not be re-proposed.

**The nine were satisfiable without executing anything, and that is a defect in the criteria rather
than an achievement to preserve.** They were written to catch missing acceptance text, unwired
edges, false `blocked` strings, unresolved disagreements, gates that name no known-bad leg, a stale
diagram, an unclosed S0 and self-referential instruments — **and they caught all eight.** What they
cannot see is whether the thing they describe ever ran. `R10` exists so that question has a row.

**DO NOT CITE A TALLY. DERIVE IT** — count the ✅/❌/⚠ glyphs in the rows above. This line went stale
**four** times in one session, most recently while the sentence beneath it was being written, and
every stale value was replaced by a fresh integer that went stale in turn. **The correction is not a
better number; it is no number** — the same discipline `AGENTS.md` applies to the package count, and
the same defect class as `m0c`'s retired `2b` and `NUMBERS.toml`'s three drifted growth counters
(`43lgt`).

**THE THREE RESIDUALS A READER MUST CARRY, all inside R9, none of them a criterion failure:**

1. **The RUN half of the gate roster has never executed** — `etyur` (P0). 13 crates declare
   `[package.metadata.gate] checks`; `derive_checks` parses them at `gate-runner/src/lib.rs:559`
   with **0** references in `main.rs`, and both `Command::new` sites are `cargo`. **A crate is
   graded `Covered{checks:true}` on a declaration nothing runs.**
2. **`l0_install` is UNMEASURABLE ON THE LANE** — the worker has no `.git`, so `identity_head()`
   cannot resolve (`~/.config/rch/config.toml` excludes `".git/"`). Green locally at 26/0 under a
   labelled bypass, cited only as the local arm of a two-arm comparison. **Fifth instance of the
   transfer class.**
3. **The full 286-invocation `--run` is untouched.** A scoped run decided R9's test half; it does
   not discharge `fsu7` item 6, and per-crate `targets=N` are **counts, not names** — attribution
   there is structurally impossible because `bounded_output` returns stdout and stderr as separate
   buffers, so every `test result:` precedes every `Running` header.

**So the honest statement is: the readiness CRITERIA are satisfied, and three named gaps remain
inside the row that measures firing.** A reader who takes "9 of 9" as "the gate suite is proven end
to end" has read this file exactly the way R9 exists to prevent.
>
> **THIS TALLY IS A TIMESTAMPED SNAPSHOT, NOT A CONSTANT.** It read `4 PASS` one minute
> before this line was written, because **R2 flipped to PASS while I was editing R4**.
> Re-derive from the rows above; never cite this integer. Same discipline as every other
> figure in this file.

> **RETRACTION, `ec7d93d`.** That commit published **R5 = FAIL** on a runner with three defects,
> two of them instrument bugs (title echo, case) and one a **definition change I made myself** —
> widening "NAMES its known-bad leg" to "names AND has fired" so that a naming criterion could be
> failed by a firing measurement. R5 is **PASS, 7 of 7**. The firing measurement was real and is
> preserved as **R9**. Caught by `%20`, which owns the correct figure and asked that R5 not be
> flipped to a firing claim on its evidence.
>
> **Ninth instrument error of the session, and the first that changed a DEFINITION rather than a
> measurement.** The others produced wrong numbers against a fixed predicate; this one moved the
> predicate, which no re-run can detect — only a reader who knows what the criterion was for.

**CORRECTED 2026-09-07 22:0xZ.** As first published this read **2/8 with R2 FAIL**. R2 was a
**wrong-key artifact** and is retracted at the criterion. R3 fell 92 → 2 under `%20`. The
remaining real failures are **R7** (S0 epic open, 4 P0 children), **R8** (instruments stale), and **R4** (8
disagreements) — plus **R5 now MEASURED as FAIL: the edges and the known-bad legs both exist; no gate has ever
fired. INERT, not unwritten.**

**Every failing criterion is planning or hygiene work — none needs new product code.** R2, R3, R4
are tracker and predicate work. R7 is two beads. R8 is a stale harvest and a worktree prune. That is
the answer to *"plan S1 fully before executing"*: the plan is not short of ideas, it is short of
**wiring, honest counts, and a closed floor.**

### R8 NARROWED 2026-09-07, and the headline is a REFUSAL

**`%19` root-caused all three components and refused to clear the one it could have.** Runner landed
at `2a9df28`; verified by pane 1 before banking.

#### THE REFUSAL, and it corrects my own dispatch

I told `%19` the harvest run was **safe**. It is safe. **It is still the wrong move**, and `%19` said
so verbatim:

> *"My interactive context CAN read those files, so a manual run would publish a digest, turn
> `fh health` GREEN, flip R8's fh component to PASS — **and the scheduled lane would still be broken
> and re-fail at 05:15 tomorrow.** That is a forged certificate, and it is worse than the red it
> replaces because it closes the question."*

**A green obtained by running the thing by hand certifies the operator, not the lane.** This is the
`.sh`-wrapper lesson in a different substrate: the mechanism must fire *where it is scheduled*, not
*where an agent can reach*.

#### The lanes EXIST and are FAILING — absent and failing have opposite remedies

Comments stripped (`crontab -l | sed 's/#.*//'`), positive control 53 real rows:
`15 5 * * * franken-harvest-daily`, `19 5 * * * fh-manifest-refresh`, `45 6 * * *
franken-harvest-health`. **My dispatch said "no cron entry" — wrong; the remedy is diagnose, never
add.**

#### TWO INDEPENDENT FAULTS, and conflating them was nearly a wrong root cause

**Fault A — a CONTEXT defect, not a data defect.** Verified by pane 1:

```
cron + launchd   MANIFEST_REFRESH_HEAD_READ_FAILED cannot read HEAD for aadc   rc=5 (both)
interactive      git -C …/dicklesworthstone-mirror/aadc rev-parse HEAD
                 -> 5a0265a06b87c442d4012cfb8af846001af168a1                  READS FINE
```

The scheduled context cannot read what an interactive shell can, corroborated by
`Operation not permitted` on another mirror path. That is **Full Disk Access / TCC for the
cron+ssh context on `/Volumes/ZestData` — a HUMAN GRANT, not an agent action.**

**Fault B is what actually blocks today's digest, and A is not it.** `fh-manifest-refresh` has failed
≥3 days *while digests published on 09-05 and 09-06*, so A cannot be the blocker.
`franken-harvest-daily`'s terminal line, verified:

```
[DRIFT] corpus denominator drift: discovered 60, expected 59 under policy franken-harvest.corpus.v1
```

**A policy compiled into another project's binary.** `%19` declined to edit another project's policy
blind, correctly.

`%19` also **refuted its own first hypothesis** — a frozen corpus correctly declining would not
produce **23 consecutive digests, 08-30..09-06.**

#### FOUR DENOMINATORS FOR ONE CORPUS

`%19` measured **219** mirror dirs; the `zeststream-rch` skill says **216**; pane 1 measures **217**;
the policy asserts **60 discovered / 59 expected**. **Four values, one corpus** — the
unstated-denominator defect at scale, and nobody should cite any of them without its glob.

#### THE LIVENESS PROBE FAILED ITS OWN POSITIVE CONTROL — so the worktree half is UNKNOWN

`lsof +D` returned **0 open fds for both candidate worktrees AND 0 for the live checkout's
`crates/`.** **A probe that reports zero on a directory five panes are actively editing has no
discriminating power**, so its zero proves nothing. `%19` refused to prune on its strength.

And it declined to prune at all, for a reason worth keeping: **pruning clears only the ABSENT entry,
taking 3 → 2 while the actual policy violation (`/private/tmp`, which `scratch-home` forbids for
durable state) survives.** A cosmetic number over a live defect — the shape R8 exists to catch.

#### TREE-CLEANLINESS IS DROPPED FROM R8. Pane 1's ruling.

111 dirty/untracked files, top writers `agent-mail-native` 8, `omp-orchestrator` 5, `ack-spine` 5,
then 3s across ten crates — **the signature of five live panes, not rot.**

> **A dirty-file count over a shared checkout with five concurrent writers is RED PRECISELY WHEN THE
> FLEET IS MOST PRODUCTIVE.** That is the docs-staleness metric and `crate-atom-gate`'s absolute
> ceilings for a third time.

R8's name is *"instruments not stale/**self-referential**"*. Tree cleanliness was never that. **Kept:
the fh half and the self-referential half — both real, neither moves with normal work.**

#### The self-referential half is DONE and its legs are TESTS, not notes

`cargo test -j 2 -p text-structure --test self_referential` → contabo-2, **exit 0, 10 passed / 0
failed**, named target per the mmt4 ruling. `prose_specimen_stripped` blanks fenced blocks and inline
spans **while preserving line structure AND byte length**, so line and column citations survive.

- **POSITIVE CONTROL:** a stripper that removed everything would make every scan vacuously green, so
  one leg proves it still finds a needle **outside** a specimen before any zero is trusted.
- **ANTI-VACUITY:** an empty hit set is a named ERROR — **and a declared rule file matching NOTHING
  is ALSO an error**, because a stale exclusion silently widens the citable set on the next edit.
- **The real-corpus leg RAN rather than declining** — verified under `--nocapture` that no
  `UNMEASURED reason=corpus_absent` was emitted, so the two documents that produced the original
  false positives were actually read on the worker.

#### One UNRUN, quoted not inferred

`cargo test -p text-structure --lib` refused twice, 45 s apart, identical mix:
`critical_pressure=1, active_project_exclusion=2, os_gate_excluded=1`. `%19` waited and retried per
the self-exclusion rule; `critical_pressure=1` persisted. **The crate's pre-existing lib unit tests
are UNRUN, not passing.**

## What actually gets us to green — measured 2026-09-07 22:4xZ by four read-only scouts

**Every remaining failure is wiring, counting, or hygiene. NOT ONE needs new product code.**

|criterion|the single action that moves it|owner|
|---|---|---|
|**R5**|**Wire the seven gates into `gate.yml`.** Their known-bad legs already exist at 3,625 chars each. INERT, not unwritten. Separately: give `djn8` a message assertion|CI lane|
|**R7**|Close S0's four P0 children, then the epic. **Read `kvsq`'s own 3,060-char acceptance first** — it carries four closing conditions and a known-bad leg, and `deps=0`, so no dependency edge encodes them|the four holders|
|**R4**|**Scope decided: wave-1 counts** (blank ≠ superseded; excluding it is an escape hatch; wave-1 is proportionally *more* resolved, 83% vs 90%). Widen the owner field's glob to all waves → `8`. Then adopt the **6 orphaned `pane4-*` rows** — 5 are one citation-hygiene batch, 1 is hook-count — and answer SilverWolf's 2|pane 1 ruled; a live pane adopts|
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

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
Measured 2026-09-07: `gate.yml` **14 jobs, 0 naming `s1-l`**; `crontab` **0**; all seven gates
`status=open`.

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
|R2 layer beads wired to their gate|✅ **PASS** — **0 of 138, STRANGLED=0**, snapshot 2026-09-07 (`rc=0`). **Was ⚠ WEAK** until `2fxd` made the predicate require *own-layer-gate* linkage, made the transpose a typed `STRANGLED` refusal, added an anti-vacuity arm, and shipped four executed fixtures. Retired predicate printed beside it on every run. History: published **FAIL 144/144** (wrong at publication — the edges predated this file by 4 days) and its first "correction" (`id`/`dependency_type`, `br show`'s keys, not the JSONL's) **also wrong**|
|R3 no false `blocked`|⚠ **NEARLY** — 2 (snapshot; was 92 → 89 → 88 → 2)|
|R4 disagreements resolved, derived count|❌ **FAIL** — **8** (wave-1: 2, wave-2: 6). Scope **RULED** wave-1 counts, three reasons intact. **`S1.toml:336`'s "(a SURVEY, not a review)" is REFUTED** — wave-1's resolutions are `accepted-and-landed`; 0 rejections because all were ACCEPTED. **6 of 8 orphaned** (`pane4-*`), 3 work items not 8|
|R5 gates NAME a known-bad leg|✅ **PASS — 7 of 7** (strict: `SUBJECT:` stripped, numbered item required). `8hq3` P0 filed by `%20`|
|R9 has any gate ever FIRED|⚠ **UNMEASURED → INERT.** 0 in CI, 0 in cron, all 7 `status=open`. **Wire it, do not write it**|
|R6 diagram receipt matches a fresh run|✅ **PASS** — 53/64, exit 0|
|R7 S0 closed|❌ **FAIL** — epic open; 4 of 7 children open, all P0 (**scope corrected: was mis-counted as 3 of 5**)|
|R8 instruments not stale/self-referential|❌ **FAIL** — fh RED, 3 worktrees|

**S1 IS NOT READY TO BUILD. 3 PASS · 2 WEAK/NEARLY · 3 FAIL · 1 INERT.**

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

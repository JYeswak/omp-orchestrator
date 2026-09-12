# ORCHESTRATOR TICK — every 5 minutes. Automated. Not a human message. DO NOT REPLY TO IT.

**You are pane 1, the conductor. Joshua's standing order: he cannot afford this project to stop, and
without this tick it stops the moment you go quiet. On 2026-09-08 the fleet sat IDLE 8h02m beside
481 ready beads and 102 P0s because the CONDUCTOR went silent, not the workers.**

**NO AUTO-DISPATCH EXISTS AND NONE IS PERMITTED. You send project-aware dispatches by hand. This
tick only guarantees you are ASKED every 5 minutes.**

## RUN THESE FIVE, IN ORDER, EVERY TICK

### 1. CHECK ALL WORKERS

```
tick-monitor observe --session omp-orchestrator
```

Never `capture-pane | grep`. Panes are **%19 idx2 · %20 idx3 · %7 idx4 · %8 idx5**.
`ntm --robot-send` takes the **INDEX**, not the `%ID`.

### 2. ANY IDLE PANE THAT DID NOT SEND A CALLBACK?

**That is the failure this tick exists to catch.** A pane has **no wake trigger**: it cannot
self-dispatch and nothing polls it, so a silent finished pane is indistinguishable from a working
one. For every `IDLE` pane, answer out loud: **did its last unit produce a `DONE` / `BLOCKED` /
`NEEDS-RULING` callback?**

- **Callback received** → it is genuinely free. Dispatch it (step 4).
- **NO callback** → it finished or died silently. **Ask it directly what happened**, and treat the
  missing callback as a defect in the packet you sent, not in the pane.

### 3. FIX THE CALLBACK CONTRACT SO THEY ACTUALLY MESSAGE BACK

**Every packet you send MUST state this, or you own the silence you get.** Measured 2026-09-02 as a
clean natural experiment: three packets carrying a stated bar produced three conforming callbacks;
the one packet that omitted it produced the only non-conforming deliverable, despite the richest
substance of the four.

```
REPLY-VIA: ntm --robot-send=omp-orchestrator --panes=1 --msg-file <path>
Fire on ALL THREE outcomes: DONE / BLOCKED / NEEDS-RULING. Do NOT batch -- report when the
unit lands, because the point is that pane 1 learns you are free AT THE MOMENT you become free.
Carry: bead id · commit sha AND its `git show HEAD:<path>` readback · BOTH remote proof lines
(`Remote command finished: exit=<N>` AND `test result:`) · NEXT: what you would pick up
unprompted · NO-CLAIM: the exact limit of what you proved · every fh/ripwire call with its
verdict class, empties included.
ONE ACK per packet using a REAL bead token: `ACK <token> on %N --`, where <token> is the LAST
hyphen-segment of a real bead id. A packet nickname can NEVER match ack-stage's exact prefix.
```

**A `BLOCKED` or `NEEDS-RULING` callback is a SUCCESS.** You cannot route around a blocker you do
not know exists.

### 4. WHAT IS NEXT IN THE PROCESS — dispatch it, project-aware, by hand

```
br ready --json --limit 0        <- returns a BARE LIST. `.get('issues')` on it THROWS and prints
                                    nothing, which reads exactly like an empty queue.
br list --status closed --json   <- `br list` EXCLUDES closed rows by default.
```

Order of value, highest first:

1. **Grade-ready beads** (`status=grading`) — route to a **NON-AUTHOR**, different **PANE**.
   Grading outruns new work: unclosed finished work makes `br ready` keep serving it, which is how a
   pane correctly reports `NO_ELIGIBLE_TARGET` and goes idle beside a full queue.
2. **Author-held beads** — release them: `br update <id> --status grading --assignee ''`. An author
   cannot grade its own work however the packet is worded.
3. **The current milestone's critical path.** Name it in the packet so the pane knows what it serves.
4. **P0s in `br ready`.**

**Re-derive every COUNT a bead's acceptance asserts BEFORE dispatching it.** 170 of 755 non-terminal
beads cite a hard count; three of three sampled were stale, and one would have authorised deleting a
live crontab executor. Verdicts: `ALREADY-FIXED` / `PREMISE-FALSE` / `STILL-LIVE`.

**Never dispatch a bead whose acceptance is UNOBTAINABLE, and note that is NOT the same as an
empty field.** Corrected 2026-09-10 by `GradePxhmd`, verified at source: the refusal is
`dispatch_packet.rs:310` (`PacketFieldMissing("acceptance")`, exit 2), **not `:166`** — which is a
bare `}`. And `acceptance()` at `:172-175` **falls back to an `ACCEPTANCE` section in the
description**, so an empty field alone does NOT refuse. It refuses only when the field is empty
**AND** the description has no `ACCEPTANCE` heading.

**Two consequences.** (1) "field is empty" and "no acceptance obtainable" are two populations, and
every dispatchability count taken on the first is the WRONG, LARGER denominator — beads whose
acceptance already sat in the description were never blocked. (2) Fill the field anyway: the
GRADER reads `acceptance_criteria`, so a field holding a NO-CLAIM paragraph while the real
`ACCEPTANCE:` sits in the description is dispatchable and still ungradeable. Three S1 beads were
measured in exactly that state.

### 5. WHAT AM *I* DOING TO DRIVE THIS PROJECT

**Answer it every tick.** If all four panes are genuinely working, you are not done — you hold your
own claimed bead and you work it. **Do not manufacture a packet to look busy**, and do not sit
watching timers.

## THE MILESTONE — RE-DERIVE IT, DO NOT CITE THIS BLOCK

**This block went stale inside one hour on 2026-09-08 and was corrected twice. It is a POINTER, not
a figure.** Run the aligner; a compile-time-baked artifact cannot tell you it is stale, and one was
measured 8 minutes older than the commit it was reporting on.

```
target/aarch64-apple-darwin/release/omp-surface-align
```

**Paste the artifact's mtime beside every `ALIGN_` line** and compare it against `git log -1`. If a
commit landed after the build, the numbers are for a tree that no longer exists.

**State as of 2026-09-08T16:20Z — the OMP surface map CLOSED at 42/42 on the last axis:**

```
cli 39/39 · rpc_handler 42/42 · rpc_notification 6/6 · transport_mode 1/1 · daemon_process 1/1
ALIGN_RESULT state=FULL citable=true · ALIGN_UNCLASSIFIED 0
RESIDUAL: ALIGN_ORPHAN_DECLARATION daemon_process:omp ps declared_by=ompo-doctor
  That axis derives from a LIVE process probe (omp-surface-align.rs:292), so its classification
  FLAPS with daemon state -- %19 predicted exactly this when it refused to add daemon_process to
  DECLARED_AXES. Driven by hqxtq: ompo ps returns DEGRADED/OMP_PS_INVALID_SHAPE on missing readyAt.
```

**HD-0049 (2026-09-08): worker topology ruled.** Adopt (A) consume ntm's robot surface — 154
`--robot-*` verbs exist, we consume 11, and 2 of 7 option-A verbs are consumed:
--robot-send-receipt (a4fb2aa) retired ack-stage unproven_transport for the NTM path and DELETED
the post-send scraper in main.rs; --robot-is-working (5c0fb65 + 15b8272) sources OMP pane state in
pane-truth and PROVED absent != idle -- a nonexistent pane returns PANE_NOT_FOUND exit=1 rather
than is_working=false, which is the collapse that let a wedged pane read as dispatchable. A missing
NTM observation is UNPROVEN. Both landed ONE verb at a time and each bounded its scraper-removal
claim to the file it touched.
⭐ **CORRECTED 2026-09-12 — THE ZERO-SET IS THREE, AND "THREE REMAIN" IS TRUE AND MISLEADING.**
`agent-health` IS consumed (two sites, reached through `ntm-kernel rate_limited_panes()` — the
kernel-only shape, where consumers name the HELPER and a verb-string search returns a FALSE ZERO).
`dialogs` IS consumed at `fast-dispatch:420`, needles stripped=0 — deleted, not left beside. The
zero-set is `answer-dialog`, `interrupt`, `inspect-pane`.
⛔ **AND `h5rr6` PROVED THE REMAINDER IS NOT ADOPTION-WITH-DELETION ACTIONABLE** (`ABSENT`, with
POSCTRL `tick-monitor/src/kernel.rs:13` proving the search CAN find a scraper, and NEGCTRL=0).
Nine live pane-text readers were enumerated and NONE asks a question `inspect-pane` answers typed
such that its scraper could be deleted in the same commit; every overlapping slice is already served
by `is-working` / `agent-health` / `dialogs`. **So adopting the three now would ADD a caller and
DELETE nothing — BUILT-not-WIRED in the other direction.** Option (A) is closer to EXHAUSTED than to
3/7 incomplete, and the residual scraping is what option (B) exists to address. **Nobody dispatches
one of these three without first naming the site whose scraper it deletes.** Spinner counts, if you
need them, carry their regex: `[Ss]pinner` over `crates/*/src` = 77 raw / 16 files / 11 crates.
Option (B), the pane-side RPC bridge is DEFERRED as the typed endgame, so **`fphs buz1 uvps jw9z djte` stay
blocked on purpose — do not force them.** (C) filed upstream.

**JOSHUA'S RULING 2026-09-08, verbatim: *"so unlock s1 and keep it going and not focus on s2."***
**S2 IS DROPPED.** Do not plan it, do not dispatch its beads, do not touch `boxes/S2.toml` or
`arc-s2-plan-gate-u4vq`. **The one exception is `gate-s1-djn8`**, because an unexecutable S1 gate
means S1 can never be marked done — that is S1 unlock work, not S2 work.

**⛔ "S1-READY" IS A READINESS GATE, NOT A DONE GATE, AND PANE 1 CONFLATED THEM ON 2026-09-08.**
`docs/plan/flow/S1-READY.md` line 1: *"the fh-backed criteria for when S1 may be BUILT."* Its ten
criteria answer **may we start**. Pane 1 closed R10, announced *"S1 IS CLOSED"*, and was wrong
within the hour — Joshua's question about S2 lock-in is what surfaced it. **Re-derive both numbers
before citing either:**

```
R1-R10  ALL TEN PASS  -- readiness only. R10 CLOSED 2026-09-08 on EXECUTION, both halves:
          fsu7  (a) CLOSED  one --run, 2857 s, 88 unique crate rows banked, set_difference=0,
                            PASS 59 / FAIL 17 / UNMEASURABLE 12, sha256 verified byte-identical
          etyur (b) CLOSED  DONE ALREADY-FIXED/PREMISE-FALSE, graded by a non-author
S1 ITSELF  NOT DONE  -- ~164 non-closed S1 beads (l0 43 · l1 5 · l2 6 · l3 24 · l4 31 · l5 30)
THE DONE-BAR lives ONLY in gate-s1-djn8's acceptance field: zero open S1 beads AND
          agreement.status == "converged" AND a refutation that CHANGED the box AND Joshua approval.
```

**AND BOTH STAGE GATES ARE CURRENTLY UNEXECUTABLE — `33ze6`.** `gate-s1-djn8` and `gate-s2-ehx8`
name `docs/plan/flow/boxes/s1.json` / `s2.json`; **neither exists** (only `S1.toml`…`S9.toml` do),
so `jq` errors and the census cannot run. **An absent input produces no verdict, and no verdict is
indistinguishable from a pass.** Also: `agreement.status = "draft"` not `converged`, `approval = ""`,
and the cited `CONTRACT.md:82-113` predicates have MOVED (approval is at `:337`). **Do NOT flip
`draft` → `converged` to make a gate pass — that is gate self-weakening.**

**THE RESIDUAL R10 REVEALED, which is NOT S1 debt. ✅ SETTLED AND ENUMERATED —
`docs/plan/flow/S1-GATE-RESIDUAL.md` ENUMERATES EVERY NON-PASS CRATE AND CARRIES THE AUTHORITATIVE
RUN. Read that doc; it is maintained. This block deliberately no longer states a count.**

⛔ **THE FIGURES THAT USED TO SIT HERE ARE GONE ON PURPOSE — THIS BLOCK WENT STALE FOUR TIMES AND
TWICE NAMED A SUPERSEDED RUN "AUTHORITATIVE", THE SECOND TIME BY THE CONDUCTOR WHO HAD JUST FIXED
THE FIRST.** A block that CONTAINS a number is wrong the moment the number moves, and this one moved
`fail=16 → 3 → 2` inside a day. **Run the producing command instead:**

```
gh run list --limit 14 --workflow=gate --json databaseId,conclusion,headSha \
  | python3 -c 'import json,sys; [print(r["databaseId"],r["conclusion"],r["headSha"][:8]) for r in json.load(sys.stdin) if r["conclusion"] in ("success","failure")]'
gh run view <ID> --log | grep -aoE "GATE_RUNNER_(FAILING|UNMEASURABLE) count=[0-9]+ names=[^ ]*|pass=[0-9]+ fail=[0-9]+ unmeasurable=[0-9]+|GATE_RUNNER_PLAN crates=[0-9]+"
```

⛔ **AND THE `PLAN crates=N` LINE IS THE PART THAT MATTERS, NOT THE FAILURE COUNT.** A FALLING
failure count is exactly what a COVERAGE COLLAPSE looks like, so `pass + fail + unmeasurable` MUST
reconcile against `GATE_RUNNER_PLAN crates=N` or the reading is worthless.

⛔ **AND THE CLAUSE THAT USED TO END THIS PARAGRAPH WAS FALSIFIED WITHIN MINUTES OF BEING WRITTEN,
BY THE CONDUCTOR WHO WROTE IT — INSIDE THE BLOCK WHOSE WHOLE POINT IS THAT FIGURES GO STALE.**
It read *"the denominator GREW 88 → 94 while failures FELL 16 → 2."* Both halves were true of the
run measured, and the very next verdict-bearing run read **`pass=87 fail=3`** with
`path-literal-guard` ENTERING the failing set. **The failure count does not fall monotonically; it
moves in BOTH directions, run to run, and a trend stated from two samples is a figure wearing a
narrative.** What survives is the RULE: the denominator is `PLAN crates=N`, and a count without it
cannot distinguish a repair from a gate that stopped looking. Read the newest run; never a trend.

Historical rows are preserved in `S1-GATE-RESIDUAL.md` as SUPERSEDED, with `fsu7`'s local bank kept
on purpose because R10 was closed against it.

**THE ORACLE IS "NEWEST run whose CONCLUSION is `success|failure`", NOT "newest completed"** —
of the 8 most recent runs, **SIX are `cancelled`**, so the lazy oracle selects a run with no
verdict at all. **The local bank is kept on purpose**: R10 was closed against it, so deleting it
would make R10's close unverifiable. **This does NOT retract R10** — R10 asked whether the layer
RAN, and it ran both times.

⚠️ **THIS BLOCK NAMED A SUPERSEDED RUN "AUTHORITATIVE" TWICE IN TWO HOURS — the second time by
the conductor who had just fixed the first.** Two agents caught it independently. **Eight crates
moved between the two readings**, so it was not drift: `omp-idle-dispatch` and
`reap-finished-panes` LEFT the failing set, while `finding-dispatch`, `installer`,
`kernel-only-operator-hook`, `ompo-doctor`, `ompo-start` and `receiver-receipt` ENTERED it.
**Six crates would have been dispatched against nothing and one repaired crate against a fixed
defect.**

⛔ **AND A CORRECT TALLY OF THE WRONG RUN IS THE MOST CONVINCING KIND OF WRONG FIGURE.** The
sum-to-88 control proves one run's internal consistency and says **nothing about which run**.
`72+12+4 = 88` is equally true at the stale head. **The control validates the tally, not the
oracle** — that is exactly what defeated both publications.

**THE DATING TELL, stronger than any timestamp comparison:** `Cargo.toml:7` is
`exclude = ["crates/omp-idle-dispatch"]` and `cargo metadata` — **the roster's own source** —
returns `ABSENT` while the directory sits on disk. It therefore CANNOT be a FAIL today; the
newest run classifies it `GATE_RUNNER_LEDGER_DRIFT`. **A FAIL row for that crate PROVES the run
predates the exclusion.** Content-derived, survives a corrupted timestamp or a relabelled id.

**`pd5ua` DELIVERED THE READING AND NOT THE ENUMERATION** — 0 of 11 failing crate names in its
comments, 2 of 11 in its body as prose. **THE NEAR-MISS IS THE REUSABLE PART:** grepping its
cited `docs/gate-roster.txt` returned **11 of 11**, because that file is the **FULL 88-crate
roster** and also contains `tick-monitor`, `pane-truth` and `bead-availability` — **all of which
PASSED**. ⛔ **A SOURCE THAT CONTAINS THE WHOLE POPULATION CANNOT EVIDENCE A SUBSET OF IT** —
same class as `grep -c ompo` → 62 counting substrings.

**AND `grep -c 'GATE_RUNNER'` IS A MOVING TARGET, WHICH IS WORSE THAN A WRONG ONE:** **4** on
`475c702`, **13** on `cb9d3941`. On the older runs the per-crate verdicts are bare
`PASS crate=… / FAIL crate=…` lines carrying **no `GATE_RUNNER` token at all**. **The newest run
ships the names directly** (`GATE_RUNNER_FAILING count=16 names=…`), so re-deriving needs no
scraping and no `sort -u`. `86zjl`'s defect, confirmed from three runs.

**AND THE SHARPER FACT, from `%20`'s `6nhj` census:** `gh run list --limit 100` returns **83
completed runs — 71 failure, 12 cancelled, ZERO success** — with **no run id or SHA cited anywhere
in `.beads/issues.jsonl`**. So the more complete measurement has been running 83 times and being
ignored, while this fleet spent 2857 s of Contabo time producing a lower-fidelity copy of it.
**That is `REACHABLE_RED_UNREAD` costing real work rather than merely being true.**
**S1 closing would not make the tree clean, and R10 passing does not mean the gates are green.**

SCOPE: R10 was THE GATE LAYER running, not the dispatcher. `gb28` holds `f3g5` item 9 as
UNRUN-pending-restart and NOBODY is to act on it.

## WHAT THIS TICK CANNOT DO

It carries **no bead id**, by design — `47g0` records that `fleet-idle-monitor`'s nudge binds panes
from `FLEET_SESSION` while reading its QUEUE from a cwd-independent `br ready`, so it hands workers a
bead from the wrong repository and self-certifies with `NUDGE_VERIFIED`. **A tick naming no bead
cannot misroute.** It cannot tell whether you are thinking or wedged, and it does not dispatch
anything — **that is your hand on every packet.**

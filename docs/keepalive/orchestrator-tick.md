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

**Never dispatch a bead whose `acceptance_criteria` is empty** — `dispatch_packet.rs:166` refuses it,
and it is ungradeable. Fill the field FIRST; a requirement living only in a packet is invisible to
every acceptance check.

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
NTM observation is UNPROVEN. FIVE remain at zero: inspect-pane, dialogs, answer-dialog, interrupt,
agent-health -- against 115 spinner-regex sites across 11 crates, tracked as qg6or. Both landed ONE
verb at a time and each bounded its scraper-removal claim to the file it touched.
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

**THE RESIDUAL R10 REVEALED, which is NOT S1 debt:** 17 crates FAIL and 12 are UNMEASURABLE against
the declared roster. That is the first real per-crate work list this repo has had. **S1 closing
would not make the tree clean, and R10 passing does not mean the gates are green.**

SCOPE: R10 was THE GATE LAYER running, not the dispatcher. `gb28` holds `f3g5` item 9 as
UNRUN-pending-restart and NOBODY is to act on it.

## WHAT THIS TICK CANNOT DO

It carries **no bead id**, by design — `47g0` records that `fleet-idle-monitor`'s nudge binds panes
from `FLEET_SESSION` while reading its QUEUE from a cwd-independent `br ready`, so it hands workers a
bead from the wrong repository and self-certifies with `NUDGE_VERIFIED`. **A tick naming no bead
cannot misroute.** It cannot tell whether you are thinking or wedged, and it does not dispatch
anything — **that is your hand on every packet.**

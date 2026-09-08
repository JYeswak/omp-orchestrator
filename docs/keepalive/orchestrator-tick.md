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

**NEXT MILESTONE: S1 — 9 of 10, and R10 is the ONLY fail.** `docs/plan/flow/S1-READY.md`:

```
R1-R9  PASS  -- and EVERY ONE IS STATIC. R10 was added BECAUSE the other nine cannot see execution:
              "Not one requires the system to run." All four lanes said so unprompted.
R10    FAIL  = two beads, both P0
  fsu7   part (a)  gate-runner banks a verdict for all 88 roster crates. VERDICT CLASS: UNRUN --
                   roster 88 rows, --run at main.rs:72, banking at lib.rs:442, all BUILT, never
                   invoked. CHUNKING IS WITHDRAWN by its own proposer: ONE run banked all 88 in
                   1289 s. Do not re-propose it.
  etyur  part (b)  PREMISE REFUTED 2026-09-08 -- the RUN HALF exists at main.rs:220-251 and says
                   so itself: "declared and executed are different facts. This loop produces the
                   second one." Needs a CLOSER, not a builder.
SCOPE: R10 is THE GATE LAYER running, not the dispatcher. gb28 holds f3g5 item 9 as
UNRUN-pending-restart and NOBODY is to act on it.
```

## WHAT THIS TICK CANNOT DO

It carries **no bead id**, by design — `47g0` records that `fleet-idle-monitor`'s nudge binds panes
from `FLEET_SESSION` while reading its QUEUE from a cwd-independent `br ready`, so it hands workers a
bead from the wrong repository and self-certifies with `NUDGE_VERIFIED`. **A tick naming no bead
cannot misroute.** It cannot tell whether you are thinking or wedged, and it does not dispatch
anything — **that is your hand on every packet.**

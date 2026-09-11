# KEEPALIVE — why this project stops, and the one line that fixes it

**Requested by Joshua 2026-09-07:** *"why are you completely incapable of keeping this project going…
ultrathink on what the game plan is that we can apply that keeps this project actually alive. work
with pane 1 of control-plane to figure out how we can properly use our installable ompo system to
drive this without stopping."*

**Bead:** `omp-orchestrator-fsu7` is adjacent (CI entry point); this file's own bead is filed below.

---

## The diagnosis, and it is not willpower

**I am a request-response process with no clock. I cannot wake myself.** A turn does not end because
I decide to stop; the turn *is* the unit, and it ends. Every doctrine line telling me "never yield
while actionable work remains" is asking for willpower over something that is not a choice.

So the question was never *"how does pane 1 avoid stopping"*. It is **"what starts the next tick, and
who owns it"**. Measured tonight, the answer was: **Joshua types something.** Every single
resumption in this session came from him.

**The product's own claim is that the conductor is a BINARY, not a pane.** `AGENTS.md`'s post-mortem
already says so: *"THE CONDUCTOR WAS A PANE. Pane 1 hand-routed work all night… the binary exists,
is cron'd, and was refused."* Tonight I re-enacted that failure — five hand-dispatches via
`ntm --robot-send`, which is precisely the handroll the KERNEL-ONLY rule forbids.

## The measurement

```
total live cron lines (comments stripped)      48
lines naming omp-orchestrator                   0     <- THIS REPO HAS NO SCHEDULER
lines naming control-plane                     32
lines invoking ANY conductor binary             0
dispatcher-deadman                              1     --observe ONLY, pointed at uds
ompo on PATH                               ABSENT     built 815f93b, never installed
```

**Nine conductor binaries are INSTALLED and none is scheduled for this repo:**
`omp-orchestrator`, `refill-idle-panes`, `fast-dispatch`, `controller-tick`, `loop-driver`,
`tick-monitor`, `fleet-monitor`, `reap-finished-panes`, `dispatcher-deadman`.

**This is a sixth executor form and the one no gate checks: SCHEDULED.** `%20` added *resident* as a
fourth beyond workflows / hooks / crontab / `Command::new`. **BUILT ≠ WIRED at the SCHEDULER level.**
`wired-but-inert-guard` runs hourly with `--check` and cannot see it, because it scans callers in
*source*, not lanes in *cron*.

## The conductor already exists, works, and verifies itself — on the wrong session

`fleet-idle-monitor.sh --nudge` runs **6×/hour**, log live, 3,273 lines:

```
IDLE_PROVEN     session=control-plane pane=%2 age=600s timer=5d
NUDGE_VERIFIED  session=control-plane pane=%2 bead=uds-x9o3 transition=omp_working_marker
```

It proves idle with **two captures**, dispatches a real bead, and **verifies the transition**. That is
exactly the receiver-receipt discipline this repo demands.

**`omp-orchestrator` hits in that log: 0.**

The wrapper is a 664-byte thin shim that states its own discipline — *"Classification, queue
selection, and nudges live in the fleet-monitor crate; this file must not grow a second keepalive
predicate"* — and `exec`s the installed Rust binary. The binary takes only
`--nudge|--report-only|--selftest` and reads exactly two env vars:

```rust
// control-plane/crates/fleet-monitor/src/bin/fleet-idle-monitor.rs:117
session: env::var("FLEET_SESSION").unwrap_or_else(|| "control-plane".to_owned())
// :103
env::var("FLEET_NUDGE_VERIFY_TIMEOUT_SECONDS")
```

**It defaults to `control-plane`. It has been working perfectly, six times an hour, on a session that
is not this one.**

## The fix, PROVEN read-only before proposal

```bash
cd /Users/josh/Developer/omp-orchestrator && FLEET_SESSION=omp-orchestrator \
  /Users/josh/Developer/control-plane/bin/fleet-idle-monitor.sh --report-only
```

Output, 0.09 s, nothing sent:

```
UNPROVEN session=omp-orchestrator pane=%5  reason=no_ready_or_working_marker   <- bare zsh, correct
WORKING  session=omp-orchestrator pane=%6  reason=omp_working_marker           <- pane 1, correct
UNPROVEN session=omp-orchestrator pane=%19 reason=first_capture
UNPROVEN session=omp-orchestrator pane=%20 reason=first_capture
UNPROVEN session=omp-orchestrator pane=%7  reason=first_capture
UNPROVEN session=omp-orchestrator pane=%8  reason=first_capture
OK no two-capture idle panes beside ready work
```

**The lane AS INSTALLED (corrected twice since first proposal):**

```cron
8,18,28,38,48,58 * * * * cd /Users/josh/Developer/omp-orchestrator && FLEET_SESSION=omp-orchestrator timeout 480 /Users/josh/.local/bin/fleet-idle-monitor --report-only >> /Users/josh/.local/state/flywheel/omp-fleet-idle.log 2>&1
```

**Two corrections are baked into that line and both were mine:**

1. **The first version invoked TWO `.sh` scripts** — `scheduled-lane-run.sh` and
   `fleet-idle-monitor.sh` — which I justified as "reuse control-plane's harness, don't invent a
   second scheduler." **Joshua: *"isn't the whole point to have the scheduler baked into ompo instead
   of using .sh scripts."*** That is this repo's ONE RULE, and I broke it by borrowing another repo's
   shell. Both are gone: the crontab header already exports the only three variables the wrapper set
   (`PATH`, `LC_ALL`, `TMUX_TMPDIR`), and `timeout` is a real binary, so the wall bound survives
   without the harness. **Lane is now 0 `.sh`.**
2. **`--nudge` → `--report-only`**, because the queue is unbound. See the retraction below.

**Still not the end state.** `fleet-idle-monitor` is **control-plane's** crate, so this repo's
keepalive depends on another repository — the same boundary error retracted at `260a3c5`. `ompo` is
now installed (`Mach-O 64-bit arm64`, 896,128 B, **86 adapters / 11 probe ids**, cross-built on
Contabo with **zero local Rust builds**), so **`ompo tick` is the candidate home** and `47g0` item 8
names it.
## ⛔ RETRACTED 2026-09-07: `cd` DOES **NOT** BIND THE QUEUE. The lane misrouted and is DISARMED.

> **This section originally claimed the `cd` was the second required half and that both halves
> together bound the queue. THE LANE PROVED OTHERWISE WITHIN THE HOUR.** Filed as
> **`omp-orchestrator-47g0`** (P0). Lane is now `--report-only`; `--nudge` removed.

**What happened, from the lane's own log:**

```
IDLE_PROVEN    session=omp-orchestrator pane=%20 age=600s timer=21d
NUDGE_VERIFIED session=omp-orchestrator pane=%20 bead=uds-snq transition=omp_working_marker
```

It classified **this** session's panes correctly and dispatched **another repository's bead** into a
live pane. `%20` refused to work it with three independent proofs: `br show uds-snq` →
`ISSUE_NOT_FOUND` with **zero** `uds-`-prefixed beads here; the acceptance demands package
`uds-fuzz` while this repo's is `omp-orchestrator-fuzz`; and `git cat-file -e 4a55b58` fails.

**THE REAL DEFECT IS TWO INDEPENDENT BINDINGS WITH ONLY ONE PARAMETERISED:**

```
fleet-idle-monitor.rs:117   env::var("FLEET_SESSION")…unwrap_or("control-plane")   <- the PANES
fleet-idle-monitor.rs:327   .args(["ready","--json","--limit","0"])                <- the QUEUE
                                                    no env var · no flag · not cwd
```

**`FLEET_SESSION` selects which panes to classify. NOTHING selects which tracker to dispatch from.**

**And `cd` provably does not fix it** — the lane carries
`cd /Users/josh/Developer/omp-orchestrator &&` for exactly this purpose, and `br ready` from that
cwd returns `omp-orchestrator-815`. **The nudge still carried `uds-snq`.** Why the binary ignores
its invocation cwd is **UNMEASURED**; `47g0` item 2 owns it.

**WHY IT IS P0: THE FAILURE IS SILENT AND SELF-CERTIFYING.** The receipt reads `NUDGE_VERIFIED`
with a real pane transition, so every liveness surface says healthy while a pane burns a turn on
work that **cannot be satisfied in the tree it is sitting in.** `%20` caught it only because it
verified the bead existed before working it — `file → claim → dispatch` catching a dispatch that
should never have been constructible.

**Fourth cross-repo instance tonight, and the first that dispatched WORK rather than a citation:**
`close-evidence-gate` doctrine borrowed into `AGENTS.md` (retracted `260a3c5`), `fleet-arc-report`
cited as in-repo when it exists only in control-plane, `8b1`'s deliverable being cross-repo by
construction — then this.

**What survives from the original section:** control-plane's own line has no `cd` either, and from
`$HOME` `br` resolves to a **third** tracker (`fc-*` prefixes). That remains a live finding for
them — but it is **not** the mechanism here, and I published it as though it were.

## What this does NOT solve — stated so nobody reads it as covered

1. **It does not make pane 1 continue, and that is the correct architecture.** The goal is not "make
   me not stop"; it is **make the fleet survive my silence**. Success is measured on the panes, not
   on me.
2. **Latency is ~2 cron intervals (≈20 min)** by design. True idle requires two captures, so the
   first tick after a pane goes idle only observes. That is the honest SLA, not a defect.
3. **IT HAS NO ORCHESTRATOR EXCLUSION — AND THAT IS THE MECHANISM, NOT THE DEFECT.** Measured:
   `fleet-idle-monitor.rs` contains **no** `orchestrator`, `exclude`, `skip`, or pane-index guard
   (positive control: 79 lines match `pane`). So it *would* nudge pane 1 — the second instance of a
   class `AGENTS.md` already records for `refill-idle-panes`, which *"proposes `pane=1`, the
   ORCHESTRATOR pane"* with no exclusion variable in its strings.

   **I filed this as a blocking risk and measurement INVERTED it.** A nudge is
   `ntm --robot-send --panes <idx> --msg <body>`, **which starts a turn in the receiving pane.** For
   every other pane that is a dispatch. **For pane 1 it is the clock I said I do not have.**
   Excluding pane 1 would delete the only thing in this design capable of waking the conductor.

   **The real residual is the PAYLOAD, not the target.** Pane 1 nudged with a *bead body* is handed
   implementation work when its job is routing — so it either works the bead (wrong role) or spends
   a turn re-routing it (acceptable, and far better than silence). **Correct end state: pane 1
   receives a CONDUCTOR TICK instruction and every other pane receives a bead.** Until that
   distinction exists, a bead nudge to pane 1 is the right stopgap and must be ALLOWED, not guarded
   against.
4. **`%5` is a bare `zsh`** and will read `UNPROVEN` forever. Either convert it to an agent or
   exclude it, or it is permanent noise in the log.
5. **A refused admission gate is silent.** `AGENTS.md`'s post-mortem records the conductor refused at
   one gate for hours while the fleet idled, and every lane fail-closes. **A lane that is
   admission-refused reads as covered and is worse than no lane.** The refusal must be loud.
6. **Crontab is shared, unbacked, single-file state** carrying 48 live lanes for four repos. Editing
   it is the highest-risk action in this plan and needs Joshua's go.

## The falsifiable success criterion

> **Panes stay working while pane 1 is silent for 30+ consecutive minutes**, with
> `NUDGE_VERIFIED session=omp-orchestrator` rows in the log proving each transition.

Not "the loop is installed." Not "cron is green." **Verified transitions on this session's panes
with no human and no pane-1 turn.** Anything less is `INERT` per gate rule 4a — and that
distinction is the whole content of tonight.

## Sequence

1. **~~Verify the orchestrator exclusion~~ — DONE, and it inverted the plan.** There is none, and
   there must not be one for pane 1. Item 3 above. No longer a blocker.
2. **Enable `--report-only` on the lane for one hour.** Confirms two-capture classification and the
   bead source without sending a packet.
3. **Ask control-plane whether the missing `cd` on its own line is intentional.** Its answer decides
   whether this is one lane or a shared fix.
4. **Flip to `--nudge`** only after 1–3, and only with Joshua's go on the crontab edit.
5. **Install `ompo`** — authorized by HD-0013, see below — so the umbrella surface exists on PATH.
6. **Point `dispatcher-deadman` at this repo** with a non-`--observe` mode, so *"eligible work
   received no packet"* is caught here and not only in uds.

## HD-0013 makes `ompo` installable today

Joshua's own decision, **recorded and unexecuted**:

> *"NO macOS SDK on the Linux lane. Split lanes by TARGET instead of cross-linking: **contabo for
> Rust/Linux compilation, the LOCAL MAC for darwin builds**, adopting zeststream-cast's working
> arrangement as prior art."*

**⚠ THE RESOLUTION BELOW IS STALE AS OF 2026-09-10 — READ THIS BEFORE ACTING ON IT.** HD-0013's
*decision* stands and its quoted text above is untouched; what is retired is the consequence drawn
from it here. <!--RETIRED-->"A binary for local `PATH` is a darwin build, which HD-0013 places on
the local Mac; the Contabo binding governs Rust/Linux compilation."<!--/RETIRED--> That inference
plus `AGENTS.md`'s *"THE POLICY IS ABSOLUTE: build on Contabo, never locally"* reads as **a Mach-O
can be built nowhere**, and on 2026-09-10 that pair stalled a live agent mid-unit. It is false:
`ompo` is on `PATH` right now as `Mach-O 64-bit executable arm64`, **cross-built on contabo with
zero local builds**. The operative clause — no macOS **SDK** on the Linux lane — is still honoured,
because the cross-build uses a **zigcc linker**, not an SDK. The working form is
`--config 'build.target="aarch64-apple-darwin"'` plus a zigcc linker `--config`, **never
`--target`** (which sets `required_os=darwin`, collapses the admissible fleet 4 → 1, and is the
`rc=103` cause); **single quotes outside, double inside**, or cargo refuses with *"string values
must be quoted."* Search `AGENTS.md` for `build.target="aarch64-apple-darwin"` rather than citing a
line number, which moves.

It remains true that **`qir1` (P0, "full macOS SDK on the contabo boxes") does the exact thing
HD-0013 rules out** — the cross-linker is the sanctioned substitute for that SDK — and my own 653 s
link failure plus `%20`'s `os_gate_excluded=3` were both us fighting a lane Joshua had already
dissolved. **`qir1`'s retirement is Joshua's call, not mine.**

## The deeper answer

**19 decisions are recorded and unexecuted** (`%7`, `a8e2fc6`, `UNEXECUTED_COUNT=19`). HD-0008 says
*push it* — 630 commits unpushed since 2026-09-02. HD-0014 says the **S1-first order is SUPERSEDED**,
which means `S1-READY.md`, the scorecard I optimised all session, may be the wrong definition of
green.

**The pattern is one thing, at three scales:** a decision with no actuator, a gate with no trigger,
and a conductor with no schedule. **Each is a mechanism that exists and is not wired to a clock.**
`lef1` is the bead for the first, `fsu7` for the second, and this file is the third.

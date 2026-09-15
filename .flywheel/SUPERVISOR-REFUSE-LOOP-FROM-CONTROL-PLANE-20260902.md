# The resident supervisor has refused 98 launches in a row, and the fix is an OPEN, UNASSIGNED P0

FROM control-plane pane 1, 2026-09-02 ~20:15Z. Read by whoever is conducting this session.

## Measured, not inferred

    launchctl print gui/501/ai.zeststream.omp-orchestrator   -> runs = 98, last exit code = 1
    heartbeat, every ~30s (ThrottleInterval):
      CYCLE_STARTED -> REAP_SWEEP_SKIPPED -> REAP_FINISHED_PANES -> GATE_UNWIRED -> SUPERVISOR_REFUSED
      detail: unwired=ack-spine[UNWIRED→repair-gate-trigger] owner=josh
    tick-monitor state: %1413 WORKING 3420s, %1414 WORKING 2220s, %1408 WORKING 1080s, %1409 WORKING 720s
    pane-truth omp-orchestrator: 4 WORKING, %1397 IDLE (orchestrator pane, excluded), %1396 IDLE (shell)

So the OBSERVE half works — tick-monitor writes state every launch — and the ACT half never runs.
Today that costs nothing because all four workers are busy. The moment one goes idle, nothing
refills it: this supervisor is the only feeder for this session, and it exits 1 before selection.

## The blocker is known and nobody holds it

`omp-orchestrator-eg0m` (P0, open, assignee=null, updated 19:17Z): *"ack-spine: the one genuinely
unwired gate — wire its invocation site or retire it."* The ruling was inverted to WIRE this
afternoon, with acceptance written (supervisor emits `StepRecord`s incl. `Closed`,
`GradeReceived`, `Redispatched`; census clears; fires-on-known-bad). It is the single edge between
this session having a conductor and not having one. Per AUTONOMOUS-WAVE.md it sits in AmberGate's
lane (gates & wiring, `%1408`).

I did not take it: it is your loop, on a shared checkout with four live workers, and the acceptance
is specific enough that the lane owner should land it. If it is still unclaimed when a worker goes
idle, that idle pane is the outage this file predicts.

## One bead I did file here

`omp-orchestrator-arur.1` (P1, under the rigor-atlas ALIGN epic): `fleet-composite` renders an
instrument failure as a dead fleet — `omp_busy: "ntm --robot-activity exited Some(1)"` coerced to
0.0 and the geometric headline printed `DEAD` every 20 minutes. The environmental cause (cron had
no `TMUX_TMPDIR`) is fixed in the control-plane crontab header; the crate defect (UNMEASURED
coerced to 0) is the bead.

## Monitors now armed from control-plane

- heartbeat: any status other than the standing ack-spine refusal (a dispatch, an idle incident,
  a new refusal reason, or the refusal clearing)
- refill-idle-panes cron lane: every apply summary and every refusal
- control-plane worker panes idle across two consecutive 10-minute captures

## Addendum 21:05Z — a selector fix sits UNCOMMITTED in this checkout; here is how to land it

`crates/refill-idle-panes/src/{lib.rs,main.rs}` carry a working-tree change by SageCastle
(Agent Mail reservation `crates/refill-idle-panes/**`, renewed to ~00:00Z): `dispatch_refusal()`
refuses `type=epic` and any status outside `open|ready` by name, `parse_recommendations_with_skips`
returns the refusals, and `--plan`/`--apply` print `SKIP bead=… reason=…`. Two planted tests. Bead:
control-plane `cp-epic-fleet-work-quality-08l6.91` (progress + build measurements in its comments).

Why it is not committed: every test build was SIGTERMed (rc=143) during rch's registry sync —
contabo-3 twice, zestdata-local once — and your pre-commit staged-build-gate needs the crate to
compile. Do NOT `git add -A` it into someone else's commit. To land: build it on a worker that
admits you (joshs-brain is running your loop-driver tests right now, so it does), run
`cargo test -p refill-idle-panes` (expect 2 new tests green), commit `-- crates/refill-idle-panes`
with a `[test]`/`[mutation]` tag, and install with your installer. The cron lane on control-plane
(`8,28,48`, now `REFILL_REPO`-pinned) picks up the new binary automatically.

If nobody lands it by the reservation's expiry, the working-tree diff is the spec; `git diff --
crates/refill-idle-panes` shows all of it.

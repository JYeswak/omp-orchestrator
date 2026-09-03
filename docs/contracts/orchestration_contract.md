# Orchestration Contract

Bead: `omp-orchestrator-orchestration-contract`

## Purpose

This contract defines what an orchestrator tick IS, the receipt every tick must emit, the six
refusals that make a tick invalid, and the all-eyes rule for every claim a tick makes. It exists
because the orchestrator role was performed by judgment for one session and failed measurably:
six operator interventions to report idle workers, fourteen badly-shaped probes, three edits to
delegated crates, three beads filed without a claim, and one build-wall self-inflicted by killing
processes on an unmeasured diagnosis.

## Contract Artifacts

1. Canonical artifact: `.flywheel/orchestration-ticks.jsonl` — one row per tick, append-only
2. Runner: `tick-monitor observe --session <s>` plus `bv --robot-triage` (both installed kernels)
3. Invariant suite: `crates/no-shell-gate/tests/orchestration_tick.rs` (fixtures preserved)
4. Callable validator and reachable binary: `orchestration-tick-gate` (`cargo run -p orchestration-tick-gate -- --ledger .flywheel/orchestration-ticks.jsonl`)

## The tick

| ID | phase | kernel — never a handroll |
|---|---|---|
| `OT-OBSERVE` | fleet + work state | `tick-monitor observe`, `bv --robot-triage` |
| `OT-SELECT` | choose next work | `bv --robot-triage` recommendations, reconciled |
| `OT-CLAIM` | project into the tracker | `br update --status in_progress --assignee` |
| `OT-DISPATCH` | send | `ntm send --panes=N` |
| `OT-RECEIPT` | confirm arrival | `tick-monitor observe` — idle→working, fresh timer |
| `OT-VERIFY` | grade landed work | re-run the cited command; never read a report |
| `OT-RECEIPT-EMIT` | write the tick row | `.flywheel/orchestration-ticks.jsonl` |

## Laws

- **OC-L1 · EVERY FREE PANE IS DISPATCHED OR NAMED.** A tick that observes `free_capacity` non-empty
  and neither dispatches nor records a per-pane refusal reason is INVALID. *"An idle worker beside
  a ready queue is the conductor's failure"* — and the failure is the tick, not the pane.
  *Test:* `every_free_pane_is_dispatched_or_refused_with_a_reason`.
- **OC-L2 · CLAIM PRECEDES DISPATCH.** No packet may name a bead that is not `in_progress` and
  assigned to the receiving agent. An unclaimed dispatch is invisible to the follow-up detector,
  which keys on *assigned + in_progress + no comment since dispatch* — so it produces **no signal
  at all**. Measured: 131 re-dispatches over 247 minutes, unnoticed. *Test:*
  `no_dispatch_row_names_an_unclaimed_bead`.
- **OC-L3 · THE ORCHESTRATOR DOES NOT DO DELEGATED WORK.** Once a crate or path is dispatched, the
  orchestrator may not edit it. Measured cost: an edit to a delegated crate, then a `cp` of HEAD
  over it to undo the damage, destroyed **889 lines** of a subagent's uncommitted rewrite.
  *Test:* `no_tick_row_edits_a_path_dispatched_in_an_earlier_row`.
- **OC-L4 · EVERY CLAIM CARRIES ITS COMMAND.** A figure without the command that produced it is
  inadmissible in a tick row. Fourteen probes were wrong in one session; each would have been
  caught by re-running the command in front of a second reader.
  *Test:* `every_measured_field_carries_a_producing_command`.
- **OC-L5 · INVESTIGATION AND ROUTING ARE NOT THE SAME TICK.** An orchestrator that begins
  diagnosing stops routing. Measured twice, a day apart: *"When pane 1 investigated the blockers,
  routing stopped and seven panes went idle."* A tick may observe and hand off; it may not descend
  into a subsystem. **Diagnosis is delegated to whoever measured it.**
  *Test:* `a_tick_row_with_an_investigation_field_also_shows_zero_free_capacity`.
- **OC-L6 · NO DESTRUCTIVE ACT WITHOUT A MEASUREMENT.** Killing, reverting, reinstalling or
  restarting requires a recorded measurement naming the target and the evidence. Measured: eight
  processes killed on "the machine is slow" with no `ps` taken — the real load was CleanMyMac at
  128% and PerfPowerServices at 186%, and the kills left stale `rchd` entries that walled off every
  build. **The remedy caused the outage.** *Test:*
  `every_destructive_action_row_cites_a_prior_measurement_row`.

## The all-eyes rule

- **OC-A1 · TWO ORACLES PER QUESTION, AND DISAGREEMENT IS AN ERROR.** Never silently prefer a
  surface. `bv --robot-next` and the ntm ready preview disagreed on the next bead; both were
  partly right and neither was checkable from its own output.
- **OC-A2 · AN EMPTY OR UNREADABLE ORACLE IS AN ERROR, NEVER AGREEMENT.** `refill-idle-panes`
  rendered a genuine two-surface conflict as `nothing to do` at exit 0 — the reassuring form of a
  refusal, which is why it survived unread for hours.
- **OC-A3 · A CONFIDENT FIELD CAN BE WRONG.** `observation_state=idle @ 0.95` and `state=idle`
  were both false about a pane 28 minutes into a turn, proven WORKING by two captures ≥75 s apart
  on both PO-L1 clauses. **No single field is sufficient**; a positive free read must be confirmed
  against the last status line at the two-capture grade.
- **OC-A4 · A REPORT IS A CLAIM.** Re-run the cited command. Of five subagent reports this session,
  **three contained corrections to the orchestrator's own figures** — the reports were more
  reliable than the briefings.

## The receipt — what makes progress measurable

One row per tick. A tick without a row did not happen.

```json
{"ts":0,"tick":0,
 "observed":{"free_capacity":[],"attention":0,"dead":0,"source":"tick-monitor observe"},
 "dispatched":[{"pane":"","bead":"","claimed_first":true,"receipt":"idle->working t=6"}],
 "refused":[{"pane":"","reason":""}],
 "landed":{"commits_since_last_tick":0,"beads_closed":0,"graded_by_non_implementer":0},
 "claims":[{"figure":"","command":""}],
 "destructive":[{"action":"","target":"","evidence_row":0}],
 "not_done":["no edits to delegated paths","no builds under freeze"]}
```

**The progress measurable is `landed.commits_since_last_tick` plus
`dispatched[] ∪ refused[] == observed.free_capacity`.** A tick where free capacity is neither
dispatched nor refused scores ZERO regardless of what else it did — which is the Rule Zero test
applied to the orchestrator instead of to the product.

## Presence — a MISSING row, and why the anchor is not in the ledger

`4wmo` made an INVALID row loud. This makes a MISSING one loud, and neither implies the other:

```
row present + invalid  ->  4wmo catches it
row absent             ->  the presence check below
```

**Measured 2026-09-02:** `.flywheel/orchestration-ticks.jsonl` was absent from disk AND HEAD
while roughly fifteen ticks ran. By this document's own words — *"a tick without a row did not
happen"* — no orchestrator tick had ever been recorded, and the six laws were gated against
fixtures the whole time. The validator checks rows that EXIST; a tick that emitted nothing
produced no row, no violation, and no signal.

### The writer

`omp-orchestrator` calls `write_tick_receipt` **once, unconditionally, immediately after
`decide()` and before the decision match**. Every arm of that match returns and several return
early, so a writer placed inside the arms is a writer the next arm inherits as absent — which is
exactly the state above. The row is built by `orchestration-tick-gate`'s own `build_receipt`, so
the crate that validates the shape also produces it, and OC-L1 holds by construction: every
observed free pane the decision did not name becomes a refusal carrying the decision class.

A figure the tick cannot measure is `null`, never `0`. `observed.dead` is null today because
`PaneObservation` carries no dead flag, and `not_done` says so in the row.

### The anchor is EXTERNAL, and tick continuity was rejected

`psf7` left this open. A **field cannot detect its own absence**: if the anchor lives in the
artifact being checked, an artifact that stopped being written has no anchor either, and an empty
ledger reads exactly like a healthy one.

Tick-number continuity is rejected for the reason the bead itself gave — a restart or a second
orchestrator legitimately breaks monotonicity, so a continuity check would fire on correct
behaviour and be disabled within a day. Measured: three of the ledger's eight rows carry
`tick=1`, because `--once` resets the counter.

The clock used instead is `write_heartbeat`'s ledger
(`~/.local/state/flywheel/omp-orchestrator.heartbeat.jsonl`), which is written on a **different
code path** from the row — so suppressing the row writer does not suppress the clock, which is
what makes a fires-on-known-bad leg possible at all.

```bash
orchestration-tick-gate --ledger .flywheel/orchestration-ticks.jsonl \
  --heartbeat ~/.local/state/flywheel/omp-orchestrator.heartbeat.jsonl
# exit 0 = PRESENT then CLEAN | exit 3 = GAP | exit 2 = the clock itself is unreadable
```

A gap is a heartbeat row whose `status` is in the declared `TICK_OUTCOME_STATUSES` allowlist and
whose `ts_unix` is newer than the newest ledger row's `ts`. `CYCLE_STARTED` is deliberately NOT an
outcome: a cycle that began owes no row yet, and a detector that fires on the healthy path gets
routed around. An unlisted future outcome **under-reports** rather than manufacturing a gap; that
direction is chosen and asserted.

An unreadable or empty clock is an ERROR, not a pass. A check that cannot run must not report
absence of a gap — that is the same silence the whole section removes.

### The division of labour, measured

A tick that ERRORS before `parse_observation` never reaches the writer. Observed tonight on a real
run: a tick ended `SUPERVISOR_REFUSED` from the outer driver, the ledger stayed at seven rows, and
`--heartbeat` answered
`TICK_ROWS_MISSING outcomes_since_last_row=1 newest_outcome=SUPERVISOR_REFUSED`.

That is the design, not a hole in it: such a row would have to **invent** an `observed` block, and
a fabricated observation in the one artifact whose purpose is auditability is worse than an absent
row that something else names. **The writer covers every tick that reached a decision; the gap
detector covers the rest.**

### NOT retro-filled

Ticks before `6bdb0a2` remain UNRECORDED in row 3's `not_done` and are not backfilled. Rows 6-8
are machine-written by ticks that actually ran on 2026-09-03; rows 1-5 remain the hand-written
ones, and the split is legible from the timestamps.

## Non-Coverage

This contract does not decide WHICH bead is next — that is `bv`'s job and `beads-north-star`'s
standard. It does not define grading (`verification_contract`), receipt semantics
(`receiver_receipt_contract`), or admission (`admission_contract`). It says nothing about
cross-session routing: panes idling in an admission-blocked repo while work exists in another is
post-mortem M3 and remains unowned.

## Validation

```bash
cargo test -p orchestration-tick-gate -- --nocapture
cargo test -p no-shell-gate --test orchestration_tick -- --nocapture
cargo run -p orchestration-tick-gate -- --ledger .flywheel/orchestration-ticks.jsonl
```

## Cross-References

- `AGENTS.md` — the four rules, KERNEL-ONLY, and the 6-hour-idle post-mortem (M1–M5)
- `docs/contracts/pane_observation_contract.md` — PO-L1 two-capture, `Unknown` inhabited
- `docs/contracts/dispatch_claim_contract.md` — file → claim → dispatch
- `docs/contracts/verification_contract.md` — bead status only; re-run, do not read
- `docs/contracts/degraded_dispatch_policy.md` — M1, dispatch under red admission
- `~/.claude/skills/project-startup/references/measured-anti-patterns.md` — 15 probe failures

## NO-CLAIM

**The receipt laws now have callable validation.** The reusable `orchestration-tick-gate` crate is
called by its binary and by the preserved `no-shell-gate` fixture suite. Its local command is the
reachable trigger; `.github/workflows/` is not used as the only trigger because this repository has
no remote. Missing or empty ledgers and invalid rows produce typed nonzero text with the ledger
path, row, and law. This does not make the ledger complete: a separate writer must ensure every
supervisor tick emits a row.

It also does not establish that these six laws are complete. They are the failures measured in one
session on one fleet; a seventh will surface, and the ledger row is the mechanism for adding it,
not a rewrite.

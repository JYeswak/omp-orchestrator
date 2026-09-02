# Kernel-Only Policy Contract

Bead: `omp-orchestrator-kernel-only-gate-wr2` (operator visibility: `omp-orchestrator-kernel-only-operator-hook-5rh`; Phase 1 contract corpus document #14)

## Purpose

This policy defines when a repository or operator action must use an existing kernel capability instead of reimplementing it: a kernel-backed handroll is a defect, a broken kernel is work to repair rather than a reason to route around, stale defect notes must be re-measured before they authorize bypass, allowlists are explicit and reasoned, and source-only scans must disclose that they cannot see operator-shell handrolls.

## Contract Artifacts

1. Canonical artifact: `crates/no-shell-gate/tests/spawn_contract.rs` — the executable source-side kernel/allowance gate; `AGENTS.md` is the policy source it enforces.
2. Runner: the single pasteable command in `## Validation` below.
3. INVARIANT SUITE: `crates/no-shell-gate/tests/spawn_contract.rs`, especially `every_spawning_crate_routes_through_the_contract_or_is_allowed`, `every_allowance_row_names_a_crate_that_still_spawns`, and `every_allowance_row_carries_a_reason`.

## Kernel-Only Model

A kernel is the existing capability that owns a boundary's correctness, scope, and evidence. The policy is a lifecycle, not a preference:

```text
kernel exists -> use it
kernel fails -> repair it
kernel note is stale -> re-measure it
exception needed -> declare and justify it
operator path differs -> disclose source-scan blind spot
```

| Capability | Stable ID | Kernel | Handroll rejected |
|---|---|---|---|
| Pane observation | `KOP-C-OBSERVE` | `tick-monitor observe` | `tmux capture-pane \| grep` used as an equivalent census. |
| Dispatch | `KOP-C-DISPATCH` | `ntm --robot-send`, `refill-idle-panes`, `fast-dispatch`, `controller-tick`, or `loop-driver` as applicable | Raw `tmux send-keys` used instead of the installed dispatch path. |
| Queue triage | `KOP-C-TRIAGE` | `bv --robot-triage` | `br ready \| python3` or another hand-parser presented as the planning brain. |
| Bead creation | `KOP-C-BEAD` | `crates/finding` | Raw `br create` when the finding workflow owns the required shape. |
| Subprocess boundary | `KOP-C-SUBPROCESS` | `subprocess-contract` | A new raw spawn equivalent when the kernel already supplies the required boundary. |

The allowlist is not inferred from “this looks harmless.” A kernel's own implementation calls are declared with a named reason; a legitimate exception is visible in the gate's output and remains subject to re-measurement.

## Laws

Each law names the existing invariant test that exercises its nearest enforceable boundary. The current tests are source-side; the operator-shell blind spot is deliberately not represented as a green source-gate result.

- **KOP-L1-KERNEL-CAPABILITY** — when a kernel provides the capability, an equivalent handroll is a defect, not a shortcut. *Test:* `crates/no-shell-gate/tests/spawn_contract.rs::every_spawning_crate_routes_through_the_contract_or_is_allowed` proves the source-side subprocess kernel route or a named allowance; the kernel-bypass scanner bead owns the analogous tmux/br/bv patterns.
- **KOP-L2-REPAIR-THE-KERNEL** — when the kernel is broken, repair the kernel or record a bounded blocker; routing around it removes the pressure that would have fixed the shared boundary. *Test:* `crates/no-shell-gate/tests/group_kill.rs::the_reference_fix_signals_the_group_and_leads_it` accepts the kernel route or an explicit group-safe implementation and rejects the private wait-deadline duplicate.
- **KOP-L3-REMEASURE-NOTES** — a stale “kernel broken” note is not an authority to bypass. A doctrine or allowance row must be re-measured against current source/runtime state before an operator follows it. *Test:* `crates/no-shell-gate/tests/spawn_contract.rs::every_allowance_row_names_a_crate_that_still_spawns` rejects an allowance whose named subject no longer exists; the same freshness obligation applies to bypass notes.
- **KOP-L4-DECLARED-ALLOWLIST** — exceptions are explicit rows with a subject and reason; kernel crates may call `tmux`, spawn children, or inspect their own boundary when that role is named. An inferred or blanket exemption is invalid. *Tests:* `crates/no-shell-gate/tests/spawn_contract.rs::every_spawning_crate_routes_through_the_contract_or_is_allowed` and `crates/no-shell-gate/tests/spawn_contract.rs::every_allowance_row_carries_a_reason`.
- **KOP-L5-OPERATOR-VISIBILITY** — a source-scan gate sees committed/tracked source only; it cannot see an operator handrolling in a shell. Its output must state that scope and identify the separate operator-hook coverage gap rather than implying full policy coverage. *Test:* `crates/no-shell-gate/tests/wired_lanes.rs::every_declared_lane_has_a_production_caller` exercises the declared source-wiring boundary; the operator hook itself remains a separate bead and is not claimed live here.

## Operating Rules

- **KOP-R1-USE-THE-NARROWEST-KERNEL** — choose the installed kernel whose contract owns the needed evidence. `tick-monitor observe` is not interchangeable with a grep because it returns state, timer, liveness, attention, dead panes, git commits, and correct session scoping.
- **KOP-R2-NO-BYPASS-BY-FAILURE** — a refusal, missing feature, or known defect in a kernel produces repair work, not a silent second implementation. If an exception is unavoidable, record its scope, reason, owner, and re-measurement condition.
- **KOP-R3-NOTE-FRESHNESS** — every “broken,” “missing,” or “not wired” note carries an observation date and must be checked against current source/runtime state before dispatching around it. A fixed note is a stale license to repeat the original handroll.
- **KOP-R4-ALLOWLIST-OUTPUT** — every exception row is emitted with its subject and reason. The gate must refuse a stale allowance, an allowance with no reason, or a spawning crate that is neither routed through `subprocess-contract` nor explicitly listed.
- **KOP-R5-SOURCE-SCOPE** — source-side results are labeled source-side. They establish nothing about commands typed by an operator outside tracked files; the operator hook is the required second observation surface.
- **KOP-R6-BUILD-SCOPE** — private build outputs belong under an owned lane below `FRANKEN_CARGO_TARGET_ROOT`, such as `CARGO_TARGET_DIR=/Volumes/BuildFH/lanes/<name>`. A `${TMPDIR}` target is refused as unowned; a build result from the wrong target is not evidence for this policy.

## Measured Failure Shape

This policy is written against the operator's measured behavior, not a hypothetical offender. The operator read panes for hours with `tmux capture-pane | grep` while the installed `tick-monitor observe` returned strictly more evidence; its first real invocation reported `gap_secs=7773`, proving that no tick had been observed for 2.2 hours. The operator hand-dispatched with raw `tmux send-keys` while `refill-idle-panes` was cron-wired, and read the queue with `br ready | python3` instead of `bv --robot-triage`.

The stale-note mechanism is equally concrete: `AGENTS.md` said `refill-idle-panes` carried only control-plane paths; that boundary was fixed on 2026-09-01 at 19:29, but the note remained and continued authorizing manual dispatch. The note was wrong by omission, not because the kernel had stopped existing.

The source gate's own limitation is part of the contract. The five observed handrolls happened in an operator shell, so a committed-source scan would pass them untouched. `kernel-only-operator-hook-5rh` is the required operator-side surface; until it is live and certified, source-gate output must say `source-only` and must not claim operator coverage.

## Non-Coverage

- No conversion of `tmux capture-pane`, `tmux send-keys`, `br ready`, or any other existing handroll in this pass.
- No implementation or certification of `kernel-only-operator-hook-5rh`; its absence is an explicit coverage gap, not a green result.
- No repair of `tick-monitor`, `refill-idle-panes`, `bv`, `ntm`, `crates/finding`, or `subprocess-contract`.
- No blanket ban on kernel implementation crates that legitimately spawn or call `tmux`; those calls require declared, reasoned allowance rows.
- No claim that a source scan sees interactive shell history, uncommitted commands, cron invocations, or other operator actions outside its scan set.
- No claim that an allowance row proves the exception is safe; it only makes the exception named and re-measurable.
- No full-workspace build or suite result; the validation command is the scoped `spawn_contract` suite only.

## Validation

```bash
RCH_ENABLED=false CARGO_MINT_MIN_CONTAINER_PCT=0 RCH_WORKER=contabo-4 CARGO_BUILD_JOBS=2 rch exec -- cargo test -j 2 -p no-shell-gate --test spawn_contract -- --nocapture
```

This runs the source-side kernel routing and allowance invariant suite without invoking the full workspace suite.

## Cross-References

- `AGENTS.md` — KERNEL-ONLY policy source and measured operator handroll table.
- `crates/no-shell-gate/tests/spawn_contract.rs` — source-side subprocess routing and explicit allowance gate.
- `crates/no-shell-gate/tests/group_kill.rs` — kernel-route/group-safety regression and private duplicate rejection.
- `crates/no-shell-gate/tests/wired_lanes.rs` — declared lane reachability and operator-hook wiring boundary.
- `crates/no-shell-gate/src/lib.rs` — no-shell gate implementation boundary.
- `crates/tick-monitor/src/lib.rs` — installed observation kernel.
- `crates/refill-idle-panes/src/lib.rs` — installed refill/dispatch kernel.
- `crates/fast-dispatch/src/lib.rs` — dispatch and queue-admission kernel boundary.
- `crates/subprocess-contract/src/lib.rs` — bounded subprocess kernel.
- `docs/contracts/subprocess_contract.md` — subprocess boundary contract.
- `docs/contracts/cancellation_contract.md` — caller-side cancellation discipline.
- `docs/plans/plan_to_write_the_document_corpus.md` — contract corpus manifest and Phase 1 order.
- `docs/contracts/asupersync_process_grade.md` — single-document pass bar.
- `/Users/josh/.claude/skills/project-startup/assets/contract-template.md` — contract shape.
- `/Users/josh/.claude/skills/project-startup/references/document-pass-bar.md` — document runner and markers.

## NO-CLAIM

A written policy does not stop a handroll. This document makes the kernel-only rule, allowlist scope, stale-note freshness, and operator blind spot explicit; enforcement still requires a live source gate and a certified operator hook. A green source-side test cannot prove that an operator chose the kernel in an interactive shell.

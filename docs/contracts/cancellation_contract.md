# Cancellation Contract

Bead: `omp-orchestrator-omp-coverage-mission-ipg.4` (Phase 1 contract corpus document #7)

## Purpose

This contract defines the cancellation discipline on the repository side of the subprocess boundary: every owned async API is region-cancellable through a first-position `&Cx`, every long operation checkpoints, every task is owned by a `Cx::spawn` or `Scope`, restrictive timeout outcomes remain restrictive, and shutdown waits are bounded so cancellation cannot become another unbounded failure mode.

## Contract Artifacts

1. Canonical artifact: the cancellation-aware source boundary in `crates/subprocess-contract/src/lib.rs`, including `run_output`, `run_status`, `checkpoint`, and the process-group configuration.
2. Runner: the single pasteable command in `## Validation` below.
3. INVARIANT SUITE: `crates/subprocess-contract/src/lib.rs` `#[cfg(test)] mod tests`, with the existing Cx, timeout, group-kill, pipe-drain, and bounded-shutdown tests cited below.

## Cancellation Model

Cancellation is ownership, not a boolean option. An async operation belongs to the caller's region, observes that region's `Cx`, checkpoints at cancellation boundaries, and returns a typed restrictive result when the deadline wins. A task that outlives its owning region is not a successful fallback; it is an ownership defect.

| Stable ID | Rule | Boundary |
|---|---|---|
| `CAN-V-CX-FIRST` | `&Cx` is the first parameter of every owned async API. | A signature the compiler can inspect. |
| `CAN-V-CHECKPOINTED` | Long loops, retries, waits, and handlers checkpoint. | A cancellation observation at each iteration or bounded phase. |
| `CAN-V-REGION-OWNED` | Tasks are spawned through `Cx::spawn` or `Scope`; detached tasks are forbidden. | The region owns task lifetime and cancellation. |
| `CAN-V-RESTRICTIVE` | `TimedOut` and other restrictive terminals never become success. | `BoundedOutcome::TimedOut` remains distinct from `Completed(Output)`. |
| `CAN-V-BOUNDED-SHUTDOWN` | Every wait, including shutdown and reader join, has a bound. | A deadline cannot turn cleanup into an infinite wait. |

## Laws

Each law names an existing test that exercises the behavior. This contract does not duplicate the subprocess kernel suite.

- **CAN-L1-CX-FIRST** — `run_output` and `run_status` take `&Cx` first; the caller's region is the cancellation authority. *Test:* `completed_child_returns_output` runs the async API under an installed Cx; the first-parameter requirement is additionally a compile-time signature property in `crates/subprocess-contract/src/lib.rs`.
- **CAN-L2-CHECKPOINT** — `cx.checkpoint()` occurs before an owned async operation and at every loop, retry, or long-handler cancellation boundary. “Long” means any operation with more than one scheduling-sized phase, any process or external I/O wait, any retry/backoff, any shutdown/reap loop, or any handler whose worst-case work is not statically bounded to one such phase. *Tests:* `completed_child_returns_output` and `both_large_pipes_are_drained_without_deadlock` exercise the Cx-aware async path; the checkpoint call is the source-level invariant.
- **CAN-L3-REGION-OWNED** — every spawned task is owned by `Cx::spawn` or a `Scope`; no detached task may survive the region that created it. *Test:* `completed_child_returns_output` exercises the region-installed async runner; the absence of detached task creation is a source-level invariant reviewed at the boundary.
- **CAN-L4-TIMEOUT-IS-NOT-A-VERDICT** — a deadline-killed child is `BoundedOutcome::TimedOut`, never `Completed(Output)` or an invented failure token. `BoundedOutcome::TimedOut` in `docs/contracts/subprocess_contract.md` is authoritative for the shared boundary. *Tests:* `sleep_child_past_deadline_is_timedout_never_completed`, `timeout_kills_the_process_group_and_is_not_a_failure_verdict`, and `bounded_status_kills_a_hung_child_and_refuses_to_call_it_completed`.
- **CAN-L5-BOUNDED-SHUTDOWN** — shutdown, reader joins, and post-kill reaping all have bounded waits; widening a task deadline must not make cleanup unbounded. *Tests:* `deadline_killed_child_is_reaped_not_orphaned`, `both_large_pipes_are_drained_without_deadlock`, and `bounded_status_kills_a_hung_child_and_refuses_to_call_it_completed`.

## Rules of Practice

- **CAN-R1-PARAMETER-ORDER** — write `async fn operation(cx: &Cx, ...)`, never `async fn operation(..., cx: &Cx)` and never an optional context hidden in a builder. The first parameter advertises cancellation ownership at every call site.
- **CAN-R2-CHECKPOINT-INTERVAL** — checkpoint before entering a cancellable phase and once per loop/retry iteration. A phase is cancellable when it waits on a process, pipe, socket, timer, lock, or external service, or when its work is not statically bounded. A wall-clock threshold alone is insufficient: a fast-looking loop can still starve cancellation under a large input.
- **CAN-R3-REGION-LIFETIME** — use `Cx::spawn` or a `Scope` so child lifetime is joined or cancelled with the parent. A detached task cannot own a lock, marker, reader, or obligation that must die with the region.
- **CAN-R4-CANCEL-THEN-CLASSIFY** — cancellation/deadline wins are classified before interpreting buffers, exit codes, or partial output. An empty buffer after a killed child is not evidence of a genuine child verdict.
- **CAN-R5-SHUTDOWN-DEADLINE** — apply a bound to every cleanup wait, including shutdown, reader join, reap, and task join. The `subprocess-contract` implementation's bounded reader join and bounded post-kill reap are the reference shape.
- **CAN-R6-DRAIN-CONCURRENTLY** — when stdout and stderr are piped, drain both concurrently while observing the child. A `try_wait()` poll with an undrained pipe can deadlock beyond roughly 64 KiB; the tell is zero CPU with no children, and increasing the timeout only hides the condition.

## Boundary Agreement

`docs/contracts/subprocess_contract.md` owns the subprocess result and process-group boundary. This contract owns the caller-side cancellation discipline. They agree on three points: `TimedOut` is restrictive and carries no child status; group TERM-then-graced-KILL is the timeout action; and no shutdown or reader wait is unbounded. If a future contract disagrees, the conflict is a contract defect that must be resolved before caller conversion; this document does not silently override the subprocess contract.

The current repository has measured debt: a `spawn_contract` report identified 15 spawning crates and approximately 144 sites without a `subprocess-contract` dependency, while `crates/no-shell-gate/tests/group_kill.rs` retains 31 pid-only `child.kill()` sites across 15 crates, including files where group and pid signaling coexist. Those are evidence for the discipline, not claims that this pass converted them.

## asupersync Shape Taken

The pinned asupersync documents provide the structural precedent:

- `crash_only_region_contract.md` supplies region lifecycle ownership, parent cancellation propagation, child-region abandonment, and checkpoint entries (`JO-OPEN-BEFORE-SPAWN`, `CR-PARENT-PROPAGATE`, `CR-CHILD-ABANDON`, `JE-CHECKPOINT`).
- `failure_domain_contract.md` supplies explicit propagation domains and restart hooks (`FD-ISOLATED`, `FD-LINKED`, `FD-ESCALATING`, `RH-PRE-RESTART`, `RH-TOMBSTONE`).

This contract transfers those boundaries without importing crash recovery, failure-domain topology, or a second task runtime into this repository.

## Non-Coverage

- No conversion of any caller to `subprocess-contract`; that is a later phase.
- No repair of the 31 pid-only kill sites, the approximately 144 un-routed spawn sites, or any individual crate's process policy.
- No refactor of `crates/omp-types`, `pane-dispatch-ready`, `fast-dispatch`, or `tick-dispatch`.
- No guarantee that a process in uninterruptible kernel state exits after TERM/KILL; the adapter must still return a bounded restrictive result.
- No claim that a source-level signature scan, green subprocess tests, or this contract proves every caller obeys the discipline.
- No definition of queue admission, pane truth, transport receipt, retry budgets, or lifecycle completion.

## Validation

```bash
cargo test -p subprocess-contract --lib -- --nocapture
```

The command runs the existing Cx-aware async tests and the subprocess-contract invariant suite for restrictive outcomes, group cleanup, concurrent pipe draining, and bounded shutdown.

## Cross-References

- `crates/subprocess-contract/src/lib.rs` — Cx-aware APIs, checkpoint, group configuration, bounded waits, and existing tests.
- `crates/subprocess-contract/Cargo.toml` — pinned asupersync dependency and crate boundary.
- `crates/no-shell-gate/tests/group_kill.rs` — known-bad pid-only-kill ratchet.
- `crates/pane-dispatch-ready/src/lib.rs` — readiness boundary and explicit no-widened-admission guard.
- `crates/fast-dispatch/src/lib.rs` — existing admission freshness and subprocess caller boundary.
- `crates/tick-dispatch/src/lib.rs` — existing ordered dispatch and subprocess caller boundary.
- `docs/contracts/subprocess_contract.md` — authoritative subprocess outcome, group-kill, pipe-drain, and stdio contract.
- `/Volumes/ZestData/dicklesworthstone-mirror/asupersync/docs/crash_only_region_contract.md` — region ownership and cancellation shape exemplar.
- `/Volumes/ZestData/dicklesworthstone-mirror/asupersync/docs/failure_domain_contract.md` — explicit propagation-domain shape exemplar.
- `docs/plans/plan_to_write_the_document_corpus.md` — contract corpus manifest and Phase 1 order.
- `docs/contracts/asupersync_process_grade.md` — single-document pass bar.

## NO-CLAIM

A written cancellation discipline converts no caller. This document names the required `&Cx`, checkpoint, region ownership, restrictive-terminal, and bounded-shutdown rules; it does not route one existing spawn through the kernel or prove that any current caller obeys them.

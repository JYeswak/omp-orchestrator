# Subprocess Contract

Bead: `omp-orchestrator-omp-coverage-mission-ipg.4` (Phase 1 contract corpus document #6)

## Purpose

This contract defines the bounded subprocess boundary exposed by `subprocess-contract`: `BoundedOutcome` has exactly three outcomes, deadline cancellation is restrictive, process groups and both output pipes are owned deliberately, and caller stdio is preserved by the status helper so a child cannot hang, be misclassified, or leave descendants holding state after the parent timed out.

## Contract Artifacts

1. Canonical artifact: `crates/subprocess-contract/src/lib.rs` — `BoundedOutcome`, `bounded_output`, and `bounded_status`.
2. Runner: the single pasteable command in `## Validation` below.
3. INVARIANT SUITE: `crates/subprocess-contract/src/lib.rs` `#[cfg(test)] mod tests`, including the named outcome, group-kill, pipe-drain, stdio, and timeout tests listed below.

## BoundedOutcome Model

`BoundedOutcome` is an exhaustive public enum. There is no `Other` or catch-all variant: adding one would permit a caller to write an arm that looks like success handling without naming the failure boundary.

| Value | Stable ID | Meaning | Restrictive |
|---|---|---|---|
| `BoundedOutcome::Completed(Output)` | `SPC-O-COMPLETED` | The child exited on its own before the deadline. `Output.status` is the only outcome field carrying a child exit status; the status may be success or failure. | No |
| `BoundedOutcome::TimedOut` | `SPC-O-TIMED-OUT` | The deadline elapsed, the process group was signalled, and the outcome carries no output or child status. | Yes |
| `BoundedOutcome::Unspawned(io::Error)` | `SPC-O-UNSPAWNED` | The child could not be spawned. The spawn error is retained; this is not a timeout. | Yes |

The async `run_output` and `run_status` helpers have the adjacent `RunError` surface because they observe an asupersync `Cx`; this three-outcome contract applies to the synchronous `bounded_output` and `bounded_status` helpers. Both surfaces must preserve the same restrictive distinction when adapted.

## Laws

Each law names the existing invariant test that exercises it. Where a test already covers the behavior, this contract cites it rather than adding a duplicate suite.

- **SPC-L1-THREE-OUTCOMES-TOTAL** — `BoundedOutcome` is exactly `Completed(Output)`, `TimedOut`, or `Unspawned(io::Error)`; no fourth `Other` outcome exists. *Test:* `sleep_child_past_deadline_is_timedout_never_completed` exhaustively matches the three variants; `bounded_status_names_an_unspawnable_command` exercises the third boundary.
- **SPC-L2-RESTRICTIVE** — `TimedOut` and `Unspawned` never become `Completed`, and only `Completed(Output)` carries a child exit status. *Tests:* `sleep_child_past_deadline_is_timedout_never_completed`, `bounded_status_kills_a_hung_child_and_refuses_to_call_it_completed`, and `bounded_status_names_an_unspawnable_command`.
- **SPC-L3-GROUP-KILL** — a bounded child is made a process-group leader with `process_group(0)`; deadline cancellation targets `-pid`, sends TERM, waits the grace interval, then sends KILL to the same group. Neither half is optional. *Tests:* `deadline_killed_child_is_reaped_not_orphaned`, `timeout_kills_the_process_group_and_is_not_a_failure_verdict`, and `bounded_status_signals_the_group_so_grandchildren_die_too`.
- **SPC-L4-DRAIN-BOTH-PIPES** — `bounded_output` reads stdout and stderr concurrently while `try_wait()` observes the child, so output beyond the pipe buffer cannot deadlock the wait. *Test:* `both_large_pipes_are_drained_without_deadlock`.
- **SPC-L5-STDIO-OWNERSHIP** — `bounded_output` explicitly pipes stdin/stdout/stderr for capture. `bounded_status` sets no stdio at all; it does not configure pipes or redirects, so caller-installed file redirects survive. “Inherited” describes the operating-system default only when the caller sets nothing; it is not an implementation action. *Test:* `bounded_status_completes_a_fast_child_with_empty_captured_output`.
- **SPC-L6-TIMEOUT-IS-NOT-A-VERDICT** — a killed child, including one whose captured buffer is empty, maps to `TimedOut`; it never becomes `Completed` or an invented failing-child token. An unspawnable child maps to `Unspawned`. *Tests:* `sleep_child_past_deadline_is_timedout_never_completed`, `timeout_kills_the_process_group_and_is_not_a_failure_verdict`, and `bounded_status_kills_a_hung_child_and_refuses_to_call_it_completed`.

## Process Rules

- **SPC-R1-CALLER-CANCELLATION** — async helpers receive `&Cx` first and checkpoint before work; cancellation is owned by the caller's region rather than a detached task.
- **SPC-R2-TERM-THEN-KILL** — timeout handling targets the child group, not only the child PID. TERM gives cooperative cleanup a bounded grace; KILL closes the restrictive path.
- **SPC-R3-BOUNDED-REAP** — reader joins and shutdown/reap waits are bounded. Increasing the deadline cannot turn a deadlocked cleanup into an unbounded wait.
- **SPC-R4-STATUS-AUTHORITY** — only `Completed(Output)` exposes `Output.status`. A killed child has no exit status to report, even if a stale or empty buffer remains available at another layer.
- **SPC-R5-KERNEL-ONLY** — callers should route new bounded subprocess work through this boundary. The existing hand-rolled call sites are conversion work for a later phase, not silently accepted alternatives.

## Failure Evidence

The contract records the failure mechanism that makes each rule load-bearing. Killing only a PID leaves grandchildren reparented to `ppid=1`; one such orphan can hold an admission lock, so the next timeout fails for the reason created by the previous timeout. Leaving either pipe undrained can block a `try_wait()` poll after roughly 64 KiB, presenting as zero CPU with no children. Widening the deadline hides both failures rather than changing them. An empty buffer after a killed child is absence of output evidence, not a child-failure verdict.

## Non-Coverage

- No migration or refactoring of callers in `pane-dispatch-ready`, `fast-dispatch`, `tick-dispatch`, or any other crate.
- No claim that the existing pid-only call sites have been converted; the current `crates/no-shell-gate/tests/group_kill.rs` census remains the ratchet for that debt.
- No process-lifetime guarantee after TERM/KILL for a child in uninterruptible kernel state; the contract bounds the adapter's wait and names the restrictive result.
- No queue admission, pane truth, freshness, transport receipt, retry policy, or lifecycle transition.
- No replacement of `RunError` with `BoundedOutcome` for the async `Cx`-aware surface.
- No authorization to treat `Completed` as application success; it only means the child exited before the adapter deadline.

## Validation

```bash
cargo test -p subprocess-contract --lib -- --nocapture
```

The command exercises the existing invariant suite, including large-pipe draining, process-group cleanup, restrictive timeout/unspawned outcomes, and the no-stdio status path.

## Cross-References

- `crates/subprocess-contract/src/lib.rs` — canonical implementation and invariant suite.
- `crates/subprocess-contract/Cargo.toml` — crate boundary and pinned asupersync dependency.
- `crates/no-shell-gate/tests/group_kill.rs` — pid-only-kill census and known-bad ratchet.
- `crates/pane-dispatch-ready/src/lib.rs` — existing readiness boundary that must not widen admission or subprocess scope here.
- `crates/fast-dispatch/src/lib.rs` — existing admission freshness boundary and subprocess caller surface.
- `crates/tick-dispatch/src/lib.rs` — existing ordered dispatch boundary and subprocess caller surface.
- `docs/plans/plan_to_write_the_document_corpus.md` — contract-corpus manifest and Phase 1 document order.
- `docs/contracts/asupersync_process_grade.md` — single-document pass bar and validation doctrine.
- `/Users/josh/.claude/skills/project-startup/assets/contract-template.md` — contract document shape.

## NO-CLAIM

A written contract does not route a single caller through the kernel. This document pins the `subprocess-contract` boundary and cites its existing tests; caller conversion, adoption receipts, and proof that every spawn uses the kernel belong to later phases.

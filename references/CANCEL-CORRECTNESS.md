# Cancellation Correctness Audit

**Consumer:** future cancellation-lint authors and reviewers of the four OMP extraction crates.
**Defect class:** line-oriented loop counts overstate cancellation defects, while sync subprocesses
can be overlooked when no `Cx` parameter is present. **Deletion condition:** remove this reference
when an in-tree gate emits the same narrowed table and its OMP-specific boundary is maintained in
that gate's output.

## Scope and distinction

Generic Asupersync rules apply to every crate: put `&Cx` first in owned async APIs, checkpoint
long loops and retry bodies, use region-owned tasks, target process groups, drain both child pipes,
and keep `TimedOut` distinct from a subject failure. Neither “abort always wins” nor “the value
always survives” is a valid general cancellation claim.

OMP-specific rules apply to the two process shapes here:

- `omp-rpc-session` drives one `omp --mode=rpc` child. Its process command creates a new process
group and targets the group for cancellation. Its protocol reader and stderr reader are run
concurrently, and `drain_bounded` drains both output paths while observing the caller's `Cx`.
- `omp-surface-consumption` is a one-shot gate CLI. It reads the installed minified bundle and
uses `subprocess-contract::bounded_output` for `command -v omp` and `omp --version`. That shared
sync helper is explicitly intended for census readers and gate bins: it has a deadline, process-
group kill, and concurrent stdout/stderr draining. The scanner is therefore **legitimately sync
at its current CLI boundary**, not a Cx-held async function. If a caller later embeds it in a
long-lived region, the boundary must become async and accept `&Cx`; adding checkpoints to its
current synchronous loops would be a category error.

## Narrowed loop table

The total column is the pane's line-oriented shape measurement, not a defect count. The two right
columns are the cancellation question: loops directly inside a function that holds `&Cx`, and the
subset lacking a checkpoint.

| crate | loops (total shape) | loops in `&Cx` functions | without checkpoint | verdict |
|---|---:|---:|---:|---|
| `ompo-doctor` | 69 | 0 | 0 | No Cx-held loop defect. `read_processes` is a separate cancellation-blind sync subprocess path. |
| `omp-rpc-session` | 9 | 4 | 0 | `await_ready` loop, `exchange_requests` outer/inner loops, and `drain_bounded` all checkpoint. |
| `omp-surface-consumption` | 15 | 0 | 0 | Synchronous one-shot scanner; no checkpoint belongs in its current loops. |
| `omp-inventory-map` | 75 | 0 | 0 | Cx-held process helper checkpoints before and after its bounded async process; its callers contain no loops. |

Structural loop matching found the four RPC loops at `await_ready`, the two `exchange_requests`
loops, and `drain_bounded`. The other loops are in synchronous helpers, tests, build support, or
functions without `&Cx`; they are not in the narrowed denominator.

## Real residual findings

### `ompo-doctor::read_processes`

`crates/ompo-doctor/src/omp_process.rs` declares:

```rust
pub async fn read_processes(cx: &asupersync::Cx, repo: &Path) -> ProcessProbeOutcome {
    let _ = cx;
    // synchronous subprocess_contract::bounded_output(...)
}
```

This is not a loop defect. It is an async API that ignores its caller's cancellation while running
the synchronous helper. The helper still has its own ten-second deadline and group-targeted cleanup,
so the residual is bounded cancellation latency rather than an unbounded hang. The doctor state
owner must repair or explicitly classify this path; this audit did not edit it.

### `omp-inventory-map::collect_surface_map_audit`

The Cx-held function calls synchronous `read_bounded_surface_map`, which performs metadata and file
reads without an intermediate checkpoint. Its file-size bound prevents unbounded retention, but the
read is not interruptible. This is a non-loop residual for the inventory-map owner, not a reason to
add checkpoints to unrelated parsing loops.

## Unwrap audit

A direct current search found seven textual `.unwrap()` calls, not the three-row shape estimate. All
seven are test-only:

- `ompo-doctor/src/provenance.rs:280` — test module, finite fixture JSON.
- `ompo-doctor/src/umbrella.rs:380` — test module, asserted capability field.
- `ompo-doctor/tests/umbrella_adapter_dispatch.rs:175` — integration test.
- `omp-inventory-map/src/types_inventory.rs:1290` — `#[cfg(test)]` fixture walker.
- `omp-inventory-map/tests/census_invariants.rs:247` — integration test serialization.
- `omp-inventory-map/tests/count_twins.rs:67` — integration test fixture mutation.
- `omp-rpc-session/tests/protocol.rs:109` — integration test after a preceding `expect`.

No production `.unwrap()` was found in these four crates. No change is warranted.

## Structural evidence

The nonexistent-root probe ran first:

```text
ripwire /nonexistent/path/xyz --report
UNKNOWN / refused: root path does not exist
```

The real `--report` scans were structural passes with no dependency cycles:

```text
ompo-doctor              22 files, 549 symbols, 554 edges, 84 modules, 0 cycles
omp-rpc-session           4 files, 131 symbols, 108 edges, 12 modules, 0 cycles
omp-surface-consumption   4 files,  56 symbols,  18 edges,  6 modules, 0 cycles
omp-inventory-map        18 files, 396 symbols, 382 edges, 52 modules, 0 cycles
```

Complete `cx.checkpoint` scans were:

```text
ompo-doctor              PASS complete=1 hits=0
omp-rpc-session           PASS complete=1 hits=2
omp-surface-consumption   PASS complete=1 hits=0
omp-inventory-map        PASS complete=1 hits=3
```

The two zero results are not merged: doctor has no Cx-held loops but has a separate sync process
residual; surface-consumption is the deliberate sync CLI boundary. The inventory and RPC positive
controls show that the checkpoint instrument can return nonzero.

`ripwire --dead-code` was invoked for all four roots, but its long default output was truncated by
the evidence channel; no dead-code claim is based on those invocations. `ripwire --dead-code --json`
returned `UNKNOWN / unsupported option`, explicitly stating JSON is not supported for that lens.
The separate AST loop probes were used for structure, not for line-oriented total counts.

## Gate-leg status

No source gate or new cancellation implementation was added in this pass because the owned crate
has zero narrowed targets in the third column. Consequently:

- fires-on-known-bad mutation: **not applicable**, no owned Cx-held loop exists to mutate;
- known-good: **observed**, RPC has four Cx-held loops and all four checkpoint; inventory has three
  checkpoint sites in its process helper;
- byte-identical restore: **not applicable**, no source edit was made;
- anti-vacuity: the absent-root ripwire control is typed `UNKNOWN`; an empty owned Cx-loop set is
  classified as the sync boundary, not silently treated as a green lint result;
- message/exit-code and wiring legs: **not applicable**, no new gate was introduced.

A future cancellation gate must make the sync-boundary classification explicit and must refuse an
empty scan set when it is claiming to audit Cx-held loops.

## OMP-specific skill delta

```text
CLAIM      Treat omp-surface-consumption as a bounded sync gate CLI today; require an async Cx
           boundary only when a caller embeds it in a cancellable region. Keep rpc-session's
           both-pipe drain and TimedOut -> UNMEASURED mapping as OMP integration requirements.
COMMAND    CARGO_TERM_COLOR=never RCH_REQUIRE_REMOTE=1 rch exec -- cargo test -j 2 -p omp-rpc-session
           --no-fail-fast
PROVENANCE omp/18.1.14; repository tree WORKTREE; this audit made no Rust source change.
```

The command is the existing RPC-session verification form and was not rerun for this read-only
cancellation audit. The skill delta is therefore a documented boundary recommendation, not a new
runtime claim. Generic cancellation doctrine remains owned by Asupersync; the OMP-specific half is
owned here only as integration guidance.

## No-claim boundary

This audit does not certify cancellation behavior under an active external abort, does not prove
that every sync file read is fast, and does not close the doctor or inventory residuals. It narrows
the loop denominator and identifies ownership. No local build, worker mutation, or process-group
experiment was performed.

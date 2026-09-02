# plan_to_pin_the_orchestrator_type_algebra

Phase 0. Named after `plan_to_port_quic_http3_to_rust.md` — a verb phrase naming ONE undertaking,
not "the plan". Convergence on this file produces
`plan_to_pin_the_orchestrator_type_algebra__after_feedback.md` beside it, never a 24th grading
round on a monolith.

## Why this, why now

Josh, 2026-09-01: *"lets start working out all types — lets follow jeff's arc properly and work
on foundational processes, regularly grading our work against the process jeffrey followed with
asupersync."*

asupersync's first five beads, `2026-01-16T06:12`–`06:14`:

```
[EPIC-PHASE] Phase 0 - Single-Thread Deterministic Kernel
Implement Outcome type with severity lattice
Implement CancelReason type with severity ordering
Implement Budget type with product semiring semantics
Implement core identifier types (RegionId, TaskId, ObligationId, Time)
```

Every task bead is *"implement `<Type>` with `<algebraic property>`"*. Those titles are only
writable because 99 `*_contract.md` files already pinned the concerns. **The contract precedes
the type; the type precedes the bead; the bead precedes the code.** That is the arc, and we have
been running it backwards — 51 crates and 142 beads against an `omp-types` crate with **zero
public types**.

## The measured hole

| crate | public types |
|---|---:|
| **`omp-types`** | **0** (129 lines) |
| `ack-spine` | 20 |
| `receiver-receipt` | 6 |
| `finding` | 5 |
| `subprocess-contract` | 2 |

The vocabulary exists — `AckVerdict`, `TransportAuthority`, `DeliveryAuthority`, `ReceiptVerdict`,
`ComposerEvidence`, `BoundedOutcome` — scattered across five crates with **no shared algebra and
no stated laws**. `omp-types`, the crate named for the job, holds none of it.

## What Phase 0 delivers

Four types, each with a contract, a law, and a property test proving the law. A type without a
stated algebraic property is a struct, not a kernel type.

### T1 · `ClaimStrength` — a total order (severity lattice)

`AGENTS.md` L1 already names it: `invariant(6) > proof(5) > bounded_model(4) > statistical(3) >
slo(2) > benchmark(1)`, with the rule **`rank(justifier) >= rank(claim)` or the build fails**.
Today that lives in prose. It is asupersync's *"Outcome type with severity lattice"* exactly.

**Law:** total order; `justifies(a, b)` iff `rank(a) >= rank(b)`; transitive; antisymmetric.

### T2 · `Admission` — a meet-semilattice, fail-closed

Every dispatch gate combines partial verdicts. Fail-closed means **any refusal dominates**:
`REFUSE ∧ anything = REFUSE`. This is the algebra our gates already assume and never state, and
it is why "one red gate makes every downstream gate UNRUN" surprised us for six hours.

**Law:** associative, commutative, idempotent; `REFUSE` is the absorbing element and `ADMIT` the
identity. Combining in any order yields the same verdict — a real property, since the fleet
combines them in whatever order the checks return.

### T3 · `PaneObservation` — evidence with a strength ordering, not a boolean

Measured tonight: a pane scored *working AND idle simultaneously* off one capture; `1h` was read
as a turn timer when it was goal-elapsed; a spinner in scrollback reported a dead pane alive.
Every one of those is a boolean standing in for graded evidence.

**Law:** two captures ≥75 s apart with a changed timer OR a changed spinner-stripped content hash
strictly dominate one capture. `Unknown` is a first-class value, never coerced to idle.

### T4 · `Lifecycle` — a state machine with RESTRICTIVE terminals

`AGENTS.md` specifies it already: `Spawned → Ready → Negotiated → Active → Stopping → Stopped`,
plus `Failed` and `TimedOut` as **restrictive** terminals a caller must never read as success.

**Law:** terminals admit no further transition (enforced by the machine, not the caller); no
`Restrictive` value converts to a success value; `BoundedOutcome::TimedOut` maps to `Restrictive`
and can never be folded into `Completed`.

## The shape each lane produces, in this order

1. **`docs/contracts/<name>_contract.md`** — target ~12 KB, his mean is 11.7 KB. One concern.
   States the type, its carrier set, its operations, its LAWS, and what it deliberately does not
   cover. **Written before the code.**
2. **The type in `omp-types`** — the shared crate, so five crates stop each inventing a dialect.
3. **A property test per law.** A law without a test is a comment.
4. **A mutation** — break the law, watch the property test go RED, restore byte-identical.

## Migration is NOT in Phase 0

Do not rewrite `ack-spine` or `receiver-receipt` to use these types yet. Phase 0 pins the algebra;
adoption is Phase 1. A lane that starts migrating will collide with four other panes in a shared
checkout, and BUILT ≠ WIRED already has us honest about the difference — Phase 1 will be the
wiring, tracked as its own beads.

## The grading cadence Josh asked for

`docs/contracts/asupersync_process_grade.md`, re-run every phase boundary, scoring us against
the measured corpus baseline:

| dimension | asupersync | ours at Phase 0 open |
|---|---:|---:|
| `docs/**.md` count / mean size | 581 / 11.7 KB | 19 / 74 KB |
| docs over 50 KB | 1% | 26% |
| `*_contract.md` | 99 | **0** |
| ADRs | 13 | **0** |
| `acceptance_criteria` on beads | 16% | 0.7% |
| `close_reason` coverage | 79% | 47% |
| distinct labels per bead | 1 per 127 | **1 per 1.2** |
| conformance / fuzz / benches dirs | 32 / 34 / 28 repos | **0 / 0 / 0** |

Every row is a command, re-derived at each grading, never quoted from this table.

## NO-CLAIM

Pinning an algebra does not make the system obey it — five crates keep their current types until
Phase 1 wires them, and a green property test proves the law holds for the type, not that any
caller respects it. The corpus baseline is one author's practice measured through a daily mirror
snapshot, and is evidence about a proven approach rather than proof of an optimum.

# Admission Contract

Bead: `omp-orchestrator-kxe.3` (Phase 0 · T2)

## Purpose

This contract defines `Admission` as the two-valued meet-semilattice used to combine dispatch-gate verdicts: `Admit` is the identity, `Refuse` is the absorbing element, and `meet` is idempotent, commutative, associative, and order-free so scheduler timing cannot change a combined admission result.

## Contract Artifacts

1. Canonical artifact: `crates/omp-types/src/lib.rs` — `Admission` and its `meet` operation.
2. Runner: the single pasteable command in `## Validation` below.
3. INVARIANT SUITE: `crates/omp-types/src/lib.rs` — `admission_law_l1_idempotent`, `admission_law_l2_commutative`, `admission_law_l3_associative_generated`, `admission_law_l4_absorbing`, `admission_law_l5_identity`, and `admission_law_l6_order_free_generated`.

## Admission Model

`Admission` is deliberately small. Gate-specific evidence, freshness, reasons, and receipts remain in the owning lifecycle records; the algebra combines only the verdict.

| Value | Stable ID | Meaning |
|---|---|---|
| `Admission::Admit` | `ADM-V-ADMIT` | This gate contributes no refusal to the combined result. |
| `Admission::Refuse` | `ADM-V-REFUSE` | This gate refuses admission; the combined result cannot become admitted. |

| Operation | Stable ID | Contract |
|---|---|---|
| `a.meet(b)` | `ADM-OP-MEET` | Returns the combined verdict without inspecting scheduling order or external state. |

Existing dispatch crates **coexist** with this type in Phase 0; they are not migrated here:

- `pane-dispatch-ready` owns `PaneDispatchReadyVerdict`, a readiness/evidence result. Its `crate_does_not_widen_admission` test explicitly keeps standing-admission policy outside that crate.
- `fast-dispatch` owns the current boolean `admission_fresh_pass` boundary: only a fresh standing PASS admits, while stale, missing, corrupt, or mismatched evidence refuses.
- `tick-dispatch` owns `TickDispatchDecision::{Allow, Refuse}` and an ordered operational check table. Its adapter boundary remains unchanged.

`Admission` is the shared algebra for later adapters, not a Phase 0 migration target. Converting those existing verdicts is Phase 1 work and must preserve their evidence and refusal reasons.

## Laws

Each law is an assertion over the canonical `meet` operation. L3 and L6 use generated inputs, not a fixed hand-picked sequence.

- **ADM-L1-IDEMPOTENT** — for every `a`, `a.meet(a) == a`. *Test:* `admission_law_l1_idempotent`.
- **ADM-L2-COMMUTATIVE** — for every `a` and `b`, `a.meet(b) == b.meet(a)`. *Test:* `admission_law_l2_commutative`.
- **ADM-L3-ASSOCIATIVE** — for every generated `a`, `b`, and `c`, `(a.meet(b)).meet(c) == a.meet(b.meet(c))`. *Test:* `admission_law_l3_associative_generated`.
- **ADM-L4-ABSORBING** — for every `a`, `Admission::Refuse.meet(a) == Admission::Refuse` and `a.meet(Admission::Refuse) == Admission::Refuse`. *Test:* `admission_law_l4_absorbing`.
- **ADM-L5-IDENTITY** — for every `a`, `Admission::Admit.meet(a) == a` and `a.meet(Admission::Admit) == a`. *Test:* `admission_law_l5_identity`.
- **ADM-L6-ORDER-FREE** — for every generated verdict vector and every generated permutation of it, folding `meet` yields the same result. *Test:* `admission_law_l6_order_free_generated`.

The generated suites use deterministic pseudo-random input generation so failures are reproducible without adding a test-only dependency. L6 includes both verdicts and their permutations; a left/right-sensitive mutation therefore changes the fold result and is observable.

## Degraded Path

- **ADM-D1-STALE-ANNOTATION** — degraded admission is an annotation on the lifecycle row, not a third `Admission` value.
- **ADM-D2-LOW-STAKES-ONLY** — the annotation is eligible only for explicitly classified low-stakes work such as grading, verification, or hygiene.
- **ADM-D3-PROVENANCE** — the lifecycle row records the stale reason, policy/work class, and bounded operator decision that allowed the degraded dispatch.
- **ADM-D4-HIGH-STAKES-BLOCK** — high-stakes dispatch requires `Admission::Admit`; a stale annotation cannot override the absorbing refusal.

This choice keeps one algebra with one meaning. Making `Stale` a second verdict would force `meet` to invent precedence between freshness and refusal and would invite callers to treat stale as admitted. A second lattice would duplicate the same gate boundary and require an unsafe coercion between lattices. An annotation preserves the fail-closed core result while making the low-stakes exception visible, bounded, and non-transferable to high-stakes work. Phase 0 defines the boundary; it does not add the lifecycle annotation field or migrate a caller.

## Validation

```bash
cargo test -p omp-types --lib admission_law_ -- --nocapture
```

The command must run all six `admission_law_*` tests. A mutation that makes refusal non-absorbing must make both `ADM-L4-ABSORBING` and `ADM-L6-ORDER-FREE` fail before the implementation is restored byte-identically.

## Cross-References

- `crates/omp-types/src/lib.rs` — canonical `Admission` implementation and invariant suite.
- `crates/pane-dispatch-ready/src/lib.rs` — readiness verdict and admission-boundary guard.
- `crates/fast-dispatch/src/lib.rs` — fresh standing-PASS admission predicate.
- `crates/tick-dispatch/src/lib.rs` — ordered operational dispatch decision.
- `crates/tick-dispatch/tests/differential.rs` — shell/Rust admission-table differential tests.
- `docs/plans/plan_to_pin_the_orchestrator_type_algebra.md` — Phase 0 type-algebra plan and T2 laws.
- `docs/contracts/asupersync_process_grade.md` — single-document contract pass bar.

## Non-Coverage

- No Phase 1 migration of `pane-dispatch-ready`, `fast-dispatch`, or `tick-dispatch`.
- No gate freshness calculation, pane truth, transport receipt, queue selection, or lifecycle-state transition.
- No refusal reason, evidence payload, timestamp, owner, or policy authority inside `Admission`.
- No authorization of degraded work; this contract only pins the annotation boundary and its restrictions.
- No claim that a combined verdict is a proof of successful packet delivery or worker completion.

## NO-CLAIM

Pinning the algebra does not make any gate obey it. The property suite proves the `Admission` type laws only; consumers remain unconverted until a later adoption phase with its own contracts, callers, and receipts.

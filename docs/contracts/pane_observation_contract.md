# PaneObservation Contract

Bead: `omp-orchestrator-phase0-pane-observation-80h`

## Purpose

Define the shared algebra for evidence about one terminal pane: a last-status-line liveness observation, its single-capture or two-capture evidence grade, and an independent dispatch-admissibility fact. The contract prevents `Unknown` from becoming `Idle`, prevents contradictory `Working` and `Idle` values, makes two-capture motion stronger than one capture, and keeps `safe_to_dispatch` outside the liveness type.

## Contract Artifacts

1. Canonical artifact: `crates/omp-types/src/pane_observation.rs` (definitions), with `crates/omp-types/src/lib.rs` providing the public module and re-exports
2. Smoke runner: `cargo test -p omp-types --test pane_observation -- --nocapture`
3. Invariant suite: `crates/omp-types/tests/pane_observation.rs` (one generated property test per law plus mutation targets)

## PaneObservation Model

`PaneObservation` contains exactly one `PaneLiveness`, one `EvidenceGrade`, and one `DispatchAdmissibility`.

| Value / ID | Meaning |
|---|---|
| `PO-UNKNOWN` | The pane cannot be classified from readable last-line evidence; it is inhabited and never coerced to idle. |
| `PO-IDLE` | The last status line carries the idle prompt state. |
| `PO-WORKING` | The last status line carries a working spinner and elapsed timer. |
| `PO-SINGLE-EVIDENCE` | One capture was classified. It is weaker than two-capture motion evidence. |
| `PO-TWO-CAPTURE-EVIDENCE` | Two captures are at least 75 seconds apart and timer or spinner-stripped content hash changed. |
| `PO-DISPATCH-UNKNOWN` | Dispatch admissibility was not established. |
| `PO-DISPATCH-ALLOWED` | An independent readiness authority permits dispatch. |
| `PO-DISPATCH-REFUSED` | An independent readiness authority refuses dispatch. |

### Constructors and operations

- `PaneObservation::unknown(pane_id, reason)` creates `PO-UNKNOWN` with `PO-DISPATCH-UNKNOWN`.
- `PaneObservation::from_last_status_line(pane_id, last_line_state, dispatch)` accepts only an already-selected last status line; it creates `PO-SINGLE-EVIDENCE`.
- `PaneObservation::from_two_captures(pane_id, previous, current, dispatch)` accepts two typed snapshots and refuses an interval below 75 seconds or unchanged timer and hash.
- `EvidenceGrade::dominates` returns true only when `PO-TWO-CAPTURE-EVIDENCE` satisfies the interval and change rule against `PO-SINGLE-EVIDENCE`.
- `PaneLiveness` is an enum, not a pair of booleans. `Working` and `Idle` therefore have one inhabited value slot and cannot coexist in one observation.

`DispatchAdmissibility` is a separate enum field. No conversion from `bool`, `safe_to_dispatch`, or a liveness variant is part of this contract.

## Laws

- **PO-L1-TWO-CAPTURE-DOMINANCE** — two captures at least 75 seconds apart with a changed timer OR changed spinner-stripped hash strictly dominate a single capture; two captures without either change do not. *Test:* `law_l1_two_capture_motion_dominates_single_capture`.
- **PO-L2-UNKNOWN-INHABITED** — unreadable or unclassifiable evidence constructs `PaneLiveness::Unknown`; no operation converts it to `Idle`. *Test:* `law_l2_unknown_never_becomes_idle`.
- **PO-L3-LAST-LINE** — the observation constructor consumes a selected last-status-line value; whole-buffer scanning is not an observation constructor. *Test:* `law_l3_constructor_uses_last_status_line_value`.
- **PO-L4-NO-CONTRADICTION** — one `PaneLiveness` enum value is exactly one of `Unknown`, `Idle`, or `Working`; `Working ∧ Idle` has no constructor. *Test:* `law_l4_liveness_is_single_exclusive_value`.
- **PO-L5-DISPATCH-SEPARATE** — changing dispatch admissibility does not change liveness or evidence grade, and unknown dispatch does not imply unknown liveness. *Test:* `law_l5_dispatch_admissibility_is_independent`.

## Evidence Ordering

`PO-TWO-CAPTURE-EVIDENCE` is stronger than `PO-SINGLE-EVIDENCE` only when its constructor's interval and change predicate pass. The ordering is evidence strength, not a claim that the pane accepted work. A two-capture `Unknown` remains `Unknown`; stronger evidence does not invent a state.

## Coexistence Boundary

Phase 0 adds the algebra to `omp-types` and does not migrate existing callers. `pane-truth` remains the terminal/status-line authority and supplies the already-selected last-line state and spinner-stripped hash. `receiver-receipt` remains the receiver-side verdict authority and retains `PanePresence`, `PostSendObservation`, `ReceiptVerdict`, and `ComposerEvidence`. The new type coexists with those six public types until a Phase 1 adoption bead proves a clean cutover.

## Non-Coverage

- No raw terminal parsing, ANSI stripping, spinner detection, timer-unit interpretation, or whole-buffer scan belongs here.
- No claim about CPU, process liveness, composer contents, queued-message state, or `safe_to_dispatch` belongs in `PaneLiveness`.
- No receiver acknowledgement, delivery proof, work comprehension, or worker completion is derived from this type.
- No migration, adapter, conversion, re-export, or caller rewrite for `pane-truth` or `receiver-receipt` is included in Phase 0.
- A single capture remains usable as a weak observation; this contract does not reject it, it prevents it from being read as the strongest evidence.

## Validation

```bash
cargo test -p omp-types --test pane_observation -- --nocapture
```

## Cross-References

- `crates/omp-types/src/pane_observation.rs` — canonical `PaneObservation` algebra
- `crates/omp-types/src/lib.rs` — public module and re-exports
- `crates/pane-truth/src/lib.rs` — last-line, timer, spinner, and capture evidence authority
- `crates/receiver-receipt/src/lib.rs` — receiver presence, post-send, receipt, and composer vocabulary
- `crates/tick-monitor/src/lib.rs` — existing observation producer and two-capture boundary
- `docs/plans/plan_to_pin_the_orchestrator_type_algebra.md` — Phase 0 T3 law
- `docs/contracts/asupersync_process_grade.md` — document grade contract

## NO-CLAIM

A correct `PaneObservation` type and green property tests prove only the stated algebra for this type. They do not fix any existing classifier: those still read terminal captures and remain Phase 1 adoption work. The type does not prove that a pane is alive, that a packet arrived, that a worker understood work, or that a dispatch is safe; those claims require their separate authorities and runtime evidence.

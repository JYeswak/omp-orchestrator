# Lifecycle Contract

Bead: `omp-orchestrator-lifecycle-type-algebra-sxz`

## Purpose

This contract defines the OMP **pane** lifecycle as a closed state machine, its transition inputs, the restrictive terminal states `Failed` and `TimedOut`, the bounded shutdown-deadline value, and the total bridge from `subprocess_contract::BoundedOutcome`; the machine owns terminal closure, timeout identity, and failure mapping so callers cannot turn an empty or deadline-killed observation into success. It deliberately pins the pane lifecycle, not the sibling RPC session lifecycle.

## Contract Artifacts

1. Canonical type: `crates/omp-types/src/lifecycle.rs`
2. Bridge authority: `crates/subprocess-contract/src/lib.rs` (`BoundedOutcome`)
3. Invariant suite: `crates/omp-types/tests/lifecycle_contract.rs`

The invariant suite is the executable contract. It contains one property test for each law and a generated transition-sequence property rather than a hand-picked happy path.

## Lifecycle Model

Every value has a stable ID. These IDs are the vocabulary used by plan sections, tests, and future consumers.

| Value | Property | Description |
|---|---|---|
| `LIFECYCLE-SPAWNED` | transient | Child/pane process has started; readiness has not been observed. |
| `LIFECYCLE-READY` | transient | Readiness was observed and the required version was advertised. |
| `LIFECYCLE-NEGOTIATED` | transient | Protocol negotiation completed successfully. |
| `LIFECYCLE-ACTIVE` | live | Handshake is complete and issued requests have been answered. |
| `LIFECYCLE-STOPPING` | transient | Shutdown was requested with a bounded deadline. |
| `LIFECYCLE-STOPPED` | terminal-success | Clean completion; no later transition is admitted. |
| `LIFECYCLE-FAILED` | restrictive-terminal | The subject failed independently or could not be spawned; never a success value. |
| `LIFECYCLE-TIMED-OUT` | restrictive-terminal | A bounded wait elapsed and the subject was killed/cancelled; never a verdict about the subject. |

### Properties

- **LIFECYCLE-P1-TERMINAL**: `Stopped`, `Failed`, and `TimedOut` are terminal; the machine returns the same state for every input.
- **LIFECYCLE-P2-RESTRICTIVE**: `Failed` and `TimedOut` cannot be converted to `Stopped`, `Active`, or any success-shaped default.
- **LIFECYCLE-P3-TIMEOUT-IDENTITY**: a timeout observation maps to `TimedOut`, even when captured output is empty.
- **LIFECYCLE-P4-DEADLINE**: shutdown input carries `WaitDeadline`; no lifecycle API constructs an unbounded wait.
- **LIFECYCLE-P5-TOTAL-BRIDGE**: every `BoundedOutcome` variant maps by an exhaustive match: successful `Completed` → `Stopped`; non-success `Completed` → `Failed`; `TimedOut` → `TimedOut`; `Unspawned` → `Failed`.

## Laws

- **L1-TERMINAL-CLOSURE** — `Stopped`, `Failed`, and `TimedOut` admit no further transition, including invalid and generated inputs. *Test:* `crates/omp-types/tests/lifecycle_contract.rs::terminal_states_are_closed_under_generated_inputs`.
- **L2-RESTRICTIVE-TERMINALS** — no operation converts `Failed` or `TimedOut` into a success value, and no `Default` or widening `From` implementation exists. *Test:* `crates/omp-types/tests/lifecycle_contract.rs::restrictive_terminals_never_become_success`.
- **L3-TIMEOUT-NOT-VERDICT** — `BoundedOutcome::TimedOut` maps to `Lifecycle::TimedOut`; an empty buffer is not parsed as a failing or completed subject. *Test:* `crates/omp-types/tests/lifecycle_contract.rs::timeout_is_not_a_subject_verdict`.
- **L4-BOUNDED-WAIT** — the shutdown transition requires a `WaitDeadline`; the deadline wrapper has no zero-argument, default, or unbounded constructor. *Test:* `crates/omp-types/tests/lifecycle_contract.rs::shutdown_input_carries_a_bounded_deadline`.
- **L5-TOTAL-BRIDGE** — the bridge matches all three `BoundedOutcome` variants and preserves restrictive terminals. *Test:* `crates/omp-types/tests/lifecycle_contract.rs::bounded_outcome_mapping_is_total`.

L1 uses generated transition sequences. The mutation leg adds an illegal terminal transition, requires L1 to go RED, then restores the source byte-identically and verifies the SHA-256.

## Authority, Recovery, and Ordering

- **LIFECYCLE-R1-STATE-AUTHORITY**: the `Lifecycle` value is authoritative for state transitions; callers receive a state, not permission to mutate it.
- **LIFECYCLE-R2-RESTRICTIVE-RECOVERY**: `Failed` and `TimedOut` may only be observed, logged, retried, or explicitly restarted by a higher-level policy; this contract never recovers them into `Active` or `Stopped`.
- **LIFECYCLE-R3-TIMEOUT-ORDERING**: timeout classification precedes output/verdict interpretation; an empty buffer after group termination remains `TimedOut`.
- **LIFECYCLE-R4-SHUTDOWN-ORDERING**: `Stopping` is entered only with a `WaitDeadline`; expiry maps to `TimedOut`, while clean completion maps to `Stopped`.

### Sibling prior art and boundary

`control-plane/crates/xtask/src/omp_rpc.rs` is the sibling-repository prior art for a typed `LifecycleMachine`, `LifecycleReport`, `TimeoutPhase`, and `FailureKind`. This contract adopts its terminal-closure and restrictive-timeout discipline, but does not copy its machine: that machine tracks the **RPC session** handshake (`Spawned → Ready → Negotiated → Active → Stopping → Stopped`), while this machine tracks an OMP **pane** and adds the `BoundedOutcome` bridge. The sibling `FailureKind` taxonomy is not treated as the pane taxonomy. AGENTS.md records that this repository consumes zero of OMP's RPC methods; therefore this contract does not claim RPC-method coverage or RPC-session wiring.

## Validation

```bash
env RCH_ENABLED=false CARGO_MINT_MIN_CONTAINER_PCT=3 FRANKEN_CARGO_TARGET_ROOT=/Volumes/ZestData/zeststream-offload-20260609/build-cache/cargo-targets /bin/bash /Users/josh/.local/bin/cargo test -p omp-types --test lifecycle_contract --offline -- --nocapture
```

## Cross-References

- `crates/omp-types/src/lifecycle.rs` — canonical pane lifecycle, deadline, and total bridge implementation
- `crates/omp-types/src/lib.rs` — canonical vocabulary re-export surface
- `crates/omp-types/tests/lifecycle_contract.rs` — five law properties and mutation target
- `crates/subprocess-contract/src/lib.rs:147-362` — bounded process outcome and process-group deadline boundary
- `crates/subprocess-contract/src/lib.rs:353-362` — `BoundedOutcome::{Completed, TimedOut, Unspawned}`
- `crates/no-shell-gate/tests/group_kill.rs` — group-kill evidence that a timeout must reach grandchildren
- `docs/contracts/asupersync_process_grade.md` — process cancellation, pipe-drain, and timeout-not-verdict grading bar
- `docs/contracts/pane_observation_contract.md` — adjacent pane observation boundary
- .flywheel/grade-evidence/lifecycle-mutation.md.gz — mutation RED and byte-identical restore transcript
- ../control-plane/crates/xtask/src/omp_rpc.rs — sibling RPC lifecycle prior art
- `docs/plans/plan_to_pin_the_orchestrator_type_algebra.md` — type-algebra plan and derivation boundary

## Non-Coverage

- This contract does not wire `Lifecycle` into `omp-orchestrator`, `tick-monitor`, launchd, tmux, or OMP RPC event consumers.
- It does not define the sibling RPC session lifecycle, OMP's 42 RPC methods, pane liveness classification, readiness admission, retry policy, or user-visible reporting.
- It does not prove that a caller uses the type, that a process was actually killed, or that a pane received a packet.
- It does not replace `subprocess-contract`'s process-group ownership, concurrent pipe draining, or cancellation implementation; it consumes that boundary's typed result.

## NO-CLAIM

A green invariant suite proves these laws for the `omp-types` lifecycle value and its local bridge only. It does not prove consumer adoption, runtime dispatch, RPC-session parity, external validation, or successful work. `Stopped` means clean termination as classified by this bridge, not business success. `Failed` and `TimedOut` remain restrictive terminals, and a timeout remains a timeout rather than a verdict about the killed subject.

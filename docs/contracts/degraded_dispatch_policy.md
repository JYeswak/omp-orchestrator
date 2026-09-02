# Degraded Dispatch Policy Contract

Bead: `omp-orchestrator-typed-degraded-dispatch-policy-i0kp`

## Purpose

This contract defines the conductor's response when the standing admission verdict is RED: low-stakes grading, verification, and hygiene work MAY dispatch with `admission=stale` recorded on its lifecycle row, while high-stakes work retains the full admission gate; it also defines fail-fast `UNRUN` semantics, the required response layer for a `DEGRADED` signal, and the rule that an idle worker beside eligible work is a conductor failure rather than worker idleness.

## Contract Artifacts

1. Canonical policy surface: `crates/omp-orchestrator/src/main.rs` — `SupervisorDecision` and the resident observation-to-dispatch match.
2. Admission surface: `crates/loop-tick/src/lib.rs` — `LoopTickRules`, `admission_pass`, and the current fail-closed tick admission path.
3. Invariant suite: `crates/omp-orchestrator/src/main.rs` `#[cfg(test)]` module, including `an_operator_declared_unavailable_reaper_yields_a_typed_skip`, `a_reasonless_skip_is_refused`, and `anything_but_the_exact_sentinel_still_fails_closed`.

The existing tests cover typed degradation for the reaper precondition. A dedicated low-stakes/high-stakes dispatch invariant suite is an adoption obligation recorded in Non-Coverage; this document does not pretend that the existing tests already enforce the complete policy.

## Degraded Dispatch Model

Every dispatch candidate has two independent classifications:

| Value | Property | Description |
|---|---|---|
| `DDP-ADMISSION-PASS` | admission | Standing admission is fresh and passing; normal dispatch policy applies. |
| `DDP-ADMISSION-RED` | admission | A known admission gate is red; the full chain is not eligible for high-stakes work. |
| `DDP-ADMISSION-UNKNOWN` | admission | Admission cannot be classified; it is not a degraded pass and cannot be silently treated as RED. |
| `DDP-LOW-STAKES` | work class | Grading, verification, or hygiene work whose correctness does not require a green tree. |
| `DDP-HIGH-STAKES` | work class | Code mutation, release, deploy, build, or other work whose acceptance requires the full green admission chain. |
| `DDP-ADMISSION-STALE` | lifecycle marker | The row records that the low-stakes dispatch proceeded while standing admission was stale/RED. |
| `DDP-GATE-UNRUN` | gate result | A downstream gate was not executed because fail-fast admission stopped the chain. It is neither PASS nor evidence that the gate itself is RED. |
| `DDP-DEGRADED-DISPATCH` | outcome | A low-stakes candidate was dispatched under `DDP-ADMISSION-RED` with `DDP-ADMISSION-STALE` recorded. |
| `DDP-DEGRADED-REFUSAL` | outcome | No authorized low-stakes candidate could dispatch; the response must retain a typed reason and nonzero outcome. |
| `DDP-CONDUCTOR-FAILURE` | ownership | Eligible work and idle capacity coexisted without dispatch; responsibility belongs to the conductor. |

### Properties

- **DDPP-CLASS-BOUNDARY**: `DDP-LOW-STAKES` and `DDP-HIGH-STAKES` are mutually exclusive for one dispatch candidate; no high-stakes candidate may be relabeled low-stakes merely to bypass admission.
- **DDPP-STALE-VISIBLE**: every `DDP-DEGRADED-DISPATCH` lifecycle row carries `admission=stale` and the red admission reason.
- **DDPP-UNRUN-NOT-RED**: `DDP-GATE-UNRUN` cannot be counted as a failure or success of that downstream gate.
- **DDPP-RESPONSE-REQUIRED**: a detected `DEGRADED` condition must reach a typed response decision, not stop at a log line or an auto-filed bead.
- **DDPP-DISPATCHABLE-ELIGIBILITY**: naming why a queue is otherwise ineligible may end in dispatch when the selected work is `DDP-LOW-STAKES` and does not require a green tree.
- **DDPP-CONDUCTOR-OWNS-IDLE**: an idle worker beside eligible ready work is a conductor defect, even when the worker itself is healthy.

## Laws

- **DDP-L1-LOW-STAKES-ESCAPE** — when admission is RED, an eligible `DDP-LOW-STAKES` bead MAY dispatch with `admission=stale` on its lifecycle row; `DDP-HIGH-STAKES` dispatch keeps the full admission gate. The degraded path is a typed class decision, not a general bypass. *Test target:* a dedicated policy invariant suite; existing typed-skip coverage is `crates/omp-orchestrator/src/main.rs::tests::an_operator_declared_unavailable_reaper_yields_a_typed_skip`.
- **DDP-L2-FAIL-FAST-UNRUN** — one RED gate makes every downstream gate `DDP-GATE-UNRUN`; the chain therefore provides no evidence about whether those downstream gates would have passed or failed. *Test target:* a dedicated fail-fast chain invariant; current admission predicate is `crates/loop-tick/src/lib.rs:49-50`.
- **DDP-L3-RESPONSE-LAYER** — detection without an authorized response is not protection. A `DEGRADED` signal must produce either `DDP-DEGRADED-DISPATCH` or a typed `DDP-DEGRADED-REFUSAL`; filing a P0 or printing a refusal alone is insufficient. *Test target:* a dedicated response-layer invariant; current refusal logging is not claimed as enforcement.
- **DDP-L4-ELIGIBILITY-CAN-DISPATCH** — the conductor's explanation of why the full queue is not eligible may end in a low-stakes dispatch when the selected bead does not need a green tree. “Named why” is not a terminal no-op. *Test target:* a dedicated queue-to-dispatch invariant.
- **DDP-L5-CONDUCTOR-ACCOUNTABILITY** — if a ready low-stakes bead and an idle eligible worker coexist without a dispatch, the failure belongs to the conductor, never to the worker. *Test target:* a dedicated idle-capacity invariant; `crates/tick-monitor/src/main.rs:513-514` already names the operational signal.

### Algebra

| Admission | Work class | Required outcome |
|---|---|---|
| PASS | LOW-STAKES or HIGH-STAKES | Normal full-gate dispatch policy. |
| RED | LOW-STAKES | `DDP-DEGRADED-DISPATCH`; lifecycle row contains `admission=stale` and the red reason. |
| RED | HIGH-STAKES | Refuse or leave `DDP-GATE-UNRUN`; never downgrade the work class. |
| RED | no eligible low-stakes work | `DDP-DEGRADED-REFUSAL` with a typed reason and nonzero outcome; no silent idle. |
| UNKNOWN/unreadable | any | The admission oracle's restrictive unknown/error contract applies; this policy does not convert it into a dispatchable RED state. |

## Authority and Response Boundary

Admission answers whether the normal high-stakes path is open. It does not own work classification, pane ground truth, queue readability, receiver receipt, or close verification. The degraded path may relax only the green-tree admission requirement for `DDP-LOW-STAKES`; it MUST preserve the independent safety and receipt checks owned by the adjacent contracts.

The response layer is part of the policy. A detector may file `cp-rjuzj` or `cp-vgine`, but the conductor must consume the signal and choose a typed dispatch or refusal outcome. Repeatedly naming the blocker while four panes remain idle beside a ready queue is not recovery.

Fail-fast status is deliberately non-transitive: `RED → UNRUN` for downstream gates, not `RED → every downstream gate is bad`. A gate that never ran supplies no verdict. The admission row must preserve both the first red gate and the downstream `UNRUN` states so later repair can run the omitted work rather than misread the chain.

## VIOLATION — where shipped code contradicts this law

- **Declared:** `DDP-L1-LOW-STAKES-ESCAPE` requires an authorized low-stakes route under a stale admission verdict.
- **Shipped:** `crates/loop-tick/src/lib.rs:49-50` requires `input.admission_pass` whenever `rules.admission_gate` is enabled; `crates/loop-tick/src/lib.rs:850` reports runtime admission refusal with no packet sent. No low-stakes class is selected in that path.
- **Consequence:** the 2026-08-31 post-mortem records 67/67 CI runs red, admission refused every tick on a stale standing verdict, four panes idle beside a ready queue, and the operator telling the orchestrator five times; detection fired but the fleet shipped no response.

- **Declared:** `DDP-L3-RESPONSE-LAYER` requires a `DEGRADED` signal to end in an authorized action or typed refusal.
- **Shipped:** `AGENTS.md:1051-1055` records that challenge-lane filed two correct P0s while no mechanism was authorized to act on `DEGRADED`; `crates/omp-orchestrator/src/main.rs:1561-1575` writes `IDLE_UNAUTHORIZED` and prints `next_action=dispatch-or-authorize` but does not itself perform the degraded dispatch.
- **Consequence:** the P0s remained open for hours while the idle/ready condition persisted.

- **Declared:** `DDP-L5-CONDUCTOR-ACCOUNTABILITY` assigns the idle-ready failure to the conductor.
- **Shipped:** the observed failure was not a dead worker; it was the conductor's admission-blocked no-packet path. `crates/tick-monitor/src/main.rs:513-514` emits the correct “DISPATCH OR GRADE NOW” signal, but detection alone does not make dispatch happen.
- **Consequence:** an idle worker beside ready work was treated as a condition to report instead of a condition to act on.

## Validation

Paste from the repository root to run the current invariant suite that contains the existing typed-degradation tests:

```bash
cargo test -p omp-orchestrator --bin omp-orchestrator -- --nocapture
```

## Cross-References

- `AGENTS.md:1026-1081` — full 2026-08-31 post-mortem and named mechanisms M1 through M5.
- `.skill-loop-progress.md:13-16` — measured `67 of 67` red CI runs.
- `crates/loop-tick/src/lib.rs` — admission predicate and refusal path.
- `crates/omp-orchestrator/src/main.rs` — supervisor decisions, typed skip tests, and current idle response.
- `crates/tick-monitor/src/main.rs` — idle-ready operational signal.
- `crates/fast-dispatch/src/main.rs` — current stale-admission refusal surface.
- `docs/contracts/oracle_comparison_contract.md` — empty/unreadable/disagreement oracle refusals; degraded dispatch does not weaken oracle truth.
- `docs/contracts/ground_truth_contract.md` — authoritative pane source and restrictive disagreement/empty-oracle boundary.
- `docs/contracts/pane_observation_contract.md` — L2: Unknown is first-class and never coerces to Idle.
- `docs/contracts/lifecycle_contract.md` — lifecycle row and typed outcome boundary.

## Non-Coverage

- This contract defines M1 only. M2 grading-lane routing, M3 cross-session routing, M4 disk-wall remediation, and M5 docs-staleness redesign remain separate mechanisms named in `AGENTS.md`.
- It does not authorize deploys, release publication, destructive actions, or high-stakes code mutation while admission is RED.
- It does not decide whether a bead is genuinely low-stakes; a separate classification implementation and invariant suite must make that decision testable without relying on labels alone.
- It does not convert `UNKNOWN`, unreadable, or stale oracle data into a green or low-stakes admission signal; `docs/contracts/oracle_comparison_contract.md` governs those refusals.
- It does not claim existing `SupervisorDecision`, `LoopTickRules`, challenge-lane, or watchdog code implements the complete M1 policy. The dedicated low/high-stakes, fail-fast, response-layer, queue-to-dispatch, and conductor-accountability tests are not yet present.
- It does not claim that dispatching low-stakes work makes the fleet healthy, proves the red gate is harmless, or repairs the underlying admission defect.

## NO-CLAIM

This document pins the policy and its failure boundary; it does not implement or wire the degraded dispatch lane. The existing typed-skip tests prove only their tested reaper sentinel behavior, not the complete M1 algebra. A future implementation must prove that low-stakes work dispatches under RED, high-stakes work remains gated, downstream `UNRUN` is preserved, a DEGRADED signal reaches an authorized response, and an idle-ready condition causes conductor action. A green validation command proves the current test suite ran; it does not prove production cron adoption, cross-session routing, or that the 2026-08-31 failure cannot recur.

# Ack Spine Contract

Bead: `omp-orchestrator-ack-spine-oj6.3`

## Purpose

Define the boundary between transport, observational delivery, and durable acknowledgement for one dispatch attempt. The contract makes each authority carry its own evidence, prevents a sender result from becoming a delivery or acknowledgement claim, preserves inhabited unknown outcomes, and requires a claimed bead before a dispatch intent can exist.

## Contract Artifacts

1. Canonical authority algebra: `crates/ack-spine/src/authorities.rs`
2. Dispatch lifecycle and durable uncertainty: `crates/ack-spine/src/spine.rs`
3. Ack and follow-up verdicts: `crates/ack-spine/src/ack.rs` and `crates/ack-spine/src/followup.rs`
4. Invariant suite: `crates/ack-spine/tests/authorities.rs`, `crates/ack-spine/tests/followup.rs`, and `crates/ack-spine/tests/spine.rs`
5. Cross-crate delivery evidence: `crates/receiver-receipt/src/lib.rs` and `docs/contracts/pane_observation_contract.md`

## Three Authorities

The three authorities answer different questions. A positive result in one column never fills either other column.

| Authority | Positive value | What it proves | What it does not prove |
|---|---|---|---|
| `AS-T-TRANSPORT` TransportAuthority | `Succeeded { receipt }` | The sending transport returned success and supplied its own receipt text. | The packet arrived, was submitted, was understood, or was acknowledged. |
| `AS-D-DELIVERY` DeliveryAuthority | `Observed { receipt: ReceiptVerdict::ReceiptConfirmed { .. } }` | An independent receiver observation supplied the canonical delivery verdict. | The tracker saw an acknowledgement or the worker understood the packet. |
| `AS-A-ACK` AckAuthority | `ReadBack { bead_id, comment_id }` | A durable tracker comment was found by read-back. | Transport success or receiver delivery. |

`AckEvidence` is the only composite evidence carrier. Its constructor receives all three authorities independently. `fully_acknowledged()` is true only when all three independent predicates are positive. `AckSummary` is a reporting projection, not evidence; its booleans may summarize an `AckEvidence` row but may not be used to manufacture one.

`ReceiptVerdict` is re-exported from `receiver-receipt`; it is a canonical cross-crate type, not one of ack-spine's 20 locally declared public types. `ComposerEvidence` remains receiver-side evidence from the same post-send capture. `ComposerEvidence::Typed` and `ComposerEvidence::Free` must not be replaced with a bare boolean or inferred from transport output.

## Public Type Inventory

The crate declares 20 local public types. The inventory is complete for the current `src/*.rs` modules; constants and functions are excluded from the type count.

| # | Type | Source | Boundary role |
|---:|---|---|---|
| 1 | `AckVerdict` | `src/ack.rs` | `Confirmed`, `Missing`, or `Unverifiable` tracker read-back result. |
| 2 | `SingularTrapResult` | `src/ack.rs` | Observes the non-posting singular `br comment` trap without treating exit status as an ack. |
| 3 | `TransportAuthority` | `src/authorities.rs` | Sender-side success/failure only. |
| 4 | `DeliveryAuthority` | `src/authorities.rs` | Independent receiver observation or named absence. |
| 5 | `AckAuthority` | `src/authorities.rs` | Durable comment read-back or named non-read-back. |
| 6 | `AckEvidence` | `src/authorities.rs` | One independently obtained fact from each authority. |
| 7 | `AckSummary` | `src/authorities.rs` | Non-authoritative boolean projection for reporting. |
| 8 | `FollowUpVerdict` | `src/followup.rs` | Typed result of checking a dispatched bead after send. |
| 9 | `FollowUpAction` | `src/followup.rs` | Whether the follow-up verdict needs operator action. |
| 10 | `HeartbeatRow` | `src/heartbeat.rs` | Third-party-readable build, process, session, repository, decision, and freshness row. |
| 11 | `LedgerError` | `src/ledger.rs` | Empty-ledger and row/step-count invariant failures. |
| 12 | `StepError` | `src/ledger.rs` | Cancellation or ledger failure from one cancel-correct step. |
| 13 | `StepKind` | `src/ledger.rs` | Closed vocabulary of dispatch lifecycle steps. |
| 14 | `StepRecord` | `src/ledger.rs` | One typed row for one step. |
| 15 | `StepLedger` | `src/ledger.rs` | Ordered rows with asserted one-row-per-step accounting. |
| 16 | `DispatchIntent` | `src/spine.rs` | Durable bead/pane/session identity for a dispatch attempt. |
| 17 | `PendingDispatch` | `src/spine.rs` | Durable uncertainty and recovery boundary. |
| 18 | `SpineError` | `src/spine.rs` | Cancel, ledger, marker, and process failures that block a positive claim. |
| 19 | `PendingDispatchStore` | `src/spine.rs` | Atomic persistence, loading, clearing, and retry blocking for uncertainty. |
| 20 | `AckSpine` | `src/spine.rs` | Coordinator that records all three authorities and finishes only on complete evidence. |

## Laws

### AS-L1-NO-UPWARD-IMPLICATION

Transport does not imply delivery. Delivery does not imply acknowledgement. The valid state `TransportAuthority::Succeeded` + `DeliveryAuthority::NotObserved` + `AckAuthority::NotReadBack` must remain constructible as an incomplete claim. `AckEvidence::new` takes the three values separately; no `From` implementation, `unwrap_or`, default, or summary projection may widen one authority into another. `AckSpine::finish` may clear pending uncertainty only after all three authorities are independently positive.

The inverse is also required: a receiver observation may be positive while the sender reports failure, and that observation must remain delivery evidence rather than being overwritten by transport failure. A retry creates another transport attempt; it does not retroactively create delivery or acknowledgement for the first attempt.

### AS-L2-TYPED-EVIDENCE

Every positive authority carries what established it:

- Transport carries the transport receipt text in `Succeeded { receipt }`.
- Delivery carries the canonical `ReceiptVerdict`; only `ReceiptConfirmed` is positive delivery evidence. `NoReceipt`, `Dead`, and `Indeterminate` remain named non-positive outcomes.
- Ack carries the bead and comment identifiers in `ReadBack { bead_id, comment_id }`.
- The receiver-side `ComposerEvidence` from the same capture remains `Typed` or `Free`, never a bare `bool`.

`AckSummary` booleans are allowed only after an `AckEvidence` row exists and are never an input authority. A typed failure or unknown reason is evidence about why a claim is not positive; it is not silently coerced to false success or to a different authority.

### AS-L3-SECOND-OBSERVATION

Delivery confirmation requires a second observation. The temporal and motion floor is owned by `docs/contracts/pane_observation_contract.md`, law `PO-L1-TWO-CAPTURE-DOMINANCE`: captures are at least 75 seconds apart and either the timer or spinner-stripped content hash changes. That contract is authoritative for the minimum evidence strength.

`receiver-receipt` is authoritative for mapping its pre/post observations to `ReceiptVerdict`, including timer reset, stable-content change, pane identity, dialog, wedged, empty-census, and unreadable cases. Its `ReceiptConfirmed` payload does not carry an explicit capture interval. Therefore an `AckSpine` caller must supply the separate `PO-TWO-CAPTURE-EVIDENCE` proof before treating `ReceiptConfirmed` as satisfying this law. A `ReceiptConfirmed` value by itself is not a substitute for the 75-second second-observation proof. This is the explicit seam where the two existing authorities do not yet encode the same fact; the pane-observation contract owns the evidence floor, and receiver-receipt owns the verdict mapping.

### AS-L4-UNKNOWN-INHABITED

Absence of an acknowledgement is not a NACK. Unknown and refusal are different inhabited outcomes:

- `AckVerdict::Missing` means the marker was not found in a readable comment list.
- `AckVerdict::Unverifiable` means the tracker or marker evidence cannot be trusted; it is an error, not `Missing`.
- `AckAuthority::NotReadBack { reason }` retains the reason rather than claiming a successful read-back.
- `ReceiptVerdict::Indeterminate` remains distinct from `NoReceipt` and `Dead`.

A follow-up model must distinguish `NoAckYet` from `AckRefused`; neither may be represented by an empty string, `false`, or a generic healthy result. The current `FollowUpVerdict` vocabulary has `Finished`, `VerdictPosted`, `SilentPastDeadline`, `Reassigned`, and `TrackerError`, but no dedicated `NoAckYet` or `AckRefused` variant. Its pre-deadline fallback currently returns `VerdictPosted`, so this law is a recorded conformance gap, not a claim that the existing classifier already enforces it. No Phase 0 contract edit changes that implementation.

### AS-L5-CLAIM-BEFORE-DISPATCH

A dispatch packet naming a bead is valid only after the bead is claimed: status is `in_progress` and the bead is assigned to the receiving agent. An unclaimed packet is a message, not a dispatch. `DispatchIntent` and `PendingDispatch` must therefore be constructible only from a claimed bead record; the claim must be part of the constructor boundary, not a caller promise. This closes the measured `5rh` failure where an unclaimed bead produced no follow-up signal because the detector keys on assigned + `in_progress`.

The current public `DispatchIntent::new(&str, &str, &str)` accepts raw identifiers and `PendingDispatch` has public fields, so the implementation does not yet enforce this law. This contract names the required boundary and records the gap for the implementation bead; it does not pretend that a permissive constructor is proof of a claim.

## Durable Recovery and Lease Receipt

`PendingDispatchStore` is the ack-spine uncertainty authority: it persists before external work, retains uncertainty after cancellation or incomplete authorities, and blocks automatic retry until an operator resolves the marker. A pending marker is not a delivery receipt and a cleared marker is not proof that a worker understood the packet.

A file reservation is a separate ownership claim. The desired lifecycle is `LEASE-HELD` → path-scoped commit observed → `LEASE-RELEASED`; the commit is the artifact-landed receipt that permits release, while the TTL remains the crash-recovery fallback. Ack-spine can express this lifecycle as a contract boundary, but it cannot enforce it: Agent Mail owns reservations and git owns commits, and neither is a type in this crate. A commit must never be treated as transport, delivery, or worker acknowledgement.

## Non-Coverage

- No transport implementation, `ntm` send, `br` comment, or receiver capture is performed by this contract.
- No contract assertion makes a sender's success response into delivery; `cp-z42vu` and its inverse remain the motivating failure modes.
- No raw terminal parsing, last-line selection, timer parsing, spinner stripping, or 75-second wait is implemented in ack-spine. Those inputs come from the separate pane-observation and receiver-receipt authorities.
- No Phase 0 migration changes `receiver-receipt`, `pane-truth`, `tick-monitor`, or existing ack-spine callers.
- No current `DispatchIntent::new` call proves bead claim state; L5 remains an implementation gap until the constructor consumes claimed tracker evidence.
- No current `FollowUpVerdict` variant proves a distinct no-ack-yet/refused split; L4 remains an implementation gap.
- Lease release, commit observation, and Agent Mail acknowledgement are outside ack-spine. The contract records the commit-aware lifecycle but does not create a lease type or release a reservation.

## Validation

```bash
python3 - <<'PY'
from pathlib import Path
import re

path = Path("docs/contracts/ack_spine_contract.md")
text = path.read_text()
checks = {
    "size": len(text.encode()) <= 25_000,
    "bead": bool(re.search(r"^Bead:", text, re.M)),
    "purpose": bool(re.search(r"^## Purpose$", text, re.M)),
    "artifacts": bool(re.search(r"^## Contract Artifacts$", text, re.M)),
    "ids": len(set(re.findall(r"\b[A-Z]{2,6}-[A-Z0-9]{2,}(?:-[A-Z0-9]+)*\b", text))) >= 5,
    "validation": bool(re.search(r"^## Validation$", text, re.M)),
    "cross_references": bool(re.search(r"^## Cross-References$", text, re.M)),
    "non_coverage": bool(re.search(r"^## Non-Coverage$", text, re.M)),
    "no_claim": bool(re.search(r"^## NO-CLAIM$", text, re.M)),
}
print(f"ACK_SPINE_CONTRACT_BYTES={len(text.encode())}")
print(checks)
if not all(checks.values()):
    raise SystemExit(1)
PY
```

## Cross-References

- `crates/ack-spine/src/authorities.rs` — three authority enums and `AckEvidence`
- `crates/ack-spine/src/ack.rs` — marker read-back and `AckVerdict`
- `crates/ack-spine/src/followup.rs` — follow-up verdict/action classifier
- `crates/ack-spine/src/spine.rs` — `DispatchIntent`, pending marker, ledger, and finish gate
- `crates/ack-spine/src/ledger.rs` — one-row-per-step ledger and cancellation boundary
- `crates/ack-spine/tests/authorities.rs` — independent-authority and mutation coverage
- `crates/ack-spine/tests/followup.rs` — deadline, reassignment, tracker-error, and typed-verdict coverage
- `crates/ack-spine/tests/spine.rs` — lifecycle, cancellation, marker, and authority coverage
- `crates/receiver-receipt/src/lib.rs` — `ReceiptVerdict`, `ComposerEvidence`, and receiver mapping
- `docs/contracts/pane_observation_contract.md` — `PO-L1-TWO-CAPTURE-DOMINANCE` evidence floor
- `docs/plans/plan_to_pin_the_orchestrator_type_algebra.md` — type-algebra direction

## NO-CLAIM

This contract defines the authority boundary and records current conformance gaps; it does not refactor ack-spine or claim that all five laws are enforced by the present constructors. In particular, it does not prove the 75-second interval is carried by `ReceiptVerdict`, that `FollowUpVerdict` distinguishes no-ack-yet from refusal, or that `DispatchIntent` can only be built from a claimed bead. It does not prove transport delivery, receiver submission, worker comprehension, tracker acknowledgement, lease release, or commit observation at runtime.

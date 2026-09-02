# Receiver Receipt Contract

Bead: `omp-orchestrator-receiver-receipt-contract-pwm`

## Purpose

Define what receiver-side evidence can establish after a transport send: two captures with the required temporal and content checks, a fresh idle-to-working transition, composer arrival without submission, and named unknown or obscured states. The contract prevents sender success, stale labels, goal timers, whole-buffer spinner matches, and one unanswered observation from becoming delivery or escalation claims.

## Contract Artifacts

1. Canonical classifier: `crates/receiver-receipt/src/lib.rs`
2. Capture adapter: `observe_capture`, which delegates last-line parsing and stable hashing to `tick-monitor`
3. Invariant suite: the `#[cfg(test)] mod tests` in `crates/receiver-receipt/src/lib.rs`, covering transition, absence, dialog, composer, and escalation cases
4. Cross-crate evidence contract: `docs/contracts/pane_observation_contract.md`, law `PO-L1-TWO-CAPTURE-DOMINANCE`
5. Delivery consumer boundary: `crates/ack-spine/src/authorities.rs` and `docs/contracts/ack_spine_contract.md`

## Evidence Model

`receiver-receipt` is observational and never sends input. The caller performs the transport action, captures the pane before and after, and supplies `PostSendObservation` to `assess_receiver_receipt`. A sender return value is not a receipt.

| Value / ID | Meaning | Does not prove |
|---|---|---|
| `RR-PANE-PRESENT` | A named pane was found in a non-empty census. | That it accepted, submitted, or understood work. |
| `RR-PANE-ABSENT` | A named pane was absent from a non-empty census. | Why it disappeared or that an empty census means all panes died. |
| `RR-PANE-LIST-EMPTY` | The census itself was empty. | Death of any pane. |
| `RR-POST-MISSING` | No post-send capture was obtained. | Non-delivery or refusal. |
| `RR-RECEIPT-CONFIRMED` | The transition-specific timer and stable-content checks support delivery. | Worker comprehension, completion, or tracker acknowledgement. |
| `RR-NO-RECEIPT` | The supplied observations support a named non-delivery reason. | A transport failure, unless transport is separately recorded. |
| `RR-INDETERMINATE` | Evidence is missing, obscured, blocked, unreadable, or otherwise insufficient. | Idle, dead, delivered, or refused. |
| `RR-COMPOSER-TYPED` | Text is present in the composer. | Submission or execution. |
| `RR-COMPOSER-FREE` | The composer is empty or carries only a greyed suggestion. | That no packet ever arrived without the matching receiver state. |

`ReceiptReason` carries the reason for `NoReceipt` or `Indeterminate`: missing observation, pane mismatch, dialog, wedged/unsubmitted composer, empty census, unreadable state, stable content, timer failure, unproven transport, or missing acknowledgement read-back. `NonDeliveryEscalation` is an action projection: `ResendDirect`, `SubmitParked`, or `KeepPolling`.

## Laws

### RR-L1-TWO-CAPTURE

A receipt requires two observations at least 75 seconds apart. The comparison must consider both the parsed elapsed timer and the spinner-stripped stable-content hash. One capture can classify a state, but it can never establish delivery.

`docs/contracts/pane_observation_contract.md` law `PO-L1-TWO-CAPTURE-DOMINANCE` is authoritative for the generic evidence-strength floor: a changed timer **or** changed spinner-stripped hash makes a valid two-capture observation stronger than one capture. This receipt contract is stricter for the final delivery claim: `ReceiptConfirmed` requires the transition-appropriate timer evidence **and** changed stable content. The two contracts agree because receipt confirmation is a narrower subset of two-capture evidence; the pane-observation contract owns evidence grade, while this contract owns delivery verdict semantics.

The current `ReceiptVerdict` payload carries timer values and stable-content status but no explicit capture interval. A caller must supply or separately validate the 75-second `PaneObservation` evidence before promoting this verdict into the `DeliveryAuthority` described by `ack_spine_contract.md`. `ReceiptConfirmed` alone is not the interval proof.

### RR-L2-FRESH-IDLE-TO-WORKING

`IDLE -> WORKING` with a fresh timer is the strongest available receiver receipt. The idle pre-state establishes that the pane was not already executing the packet; the working post-state establishes a new turn; the small timer bounds the transition as fresh; and the changed stable-content hash shows that the pane's non-animated content changed. The measured shape is `%1413 IDLE -> WORKING t=17s`.

This remains a receiver observation, not a comprehension claim. It does not prove the worker parsed the requested bead, accepted the requested objective, completed the work, posted a grade, or wrote an acknowledgement. The `MAX_IDLE_TO_WORKING_TIMER_SECS` bound is 30 seconds in the current classifier; it is not a substitute for the separate 75-second two-capture evidence floor.

### RR-L3-COMPOSER-ARRIVAL-NOT-DELIVERY

`ComposerEvidence::Typed` establishes that text is present in the composer for the same post-send capture. It may mean the packet arrived but was never submitted; it does not establish acceptance or execution. The parked state is the measured `Press up to edit queued messages` condition.

`ComposerEvidence::Free` with an idle pane after a sender success supports `NonDeliveryEscalation::ResendDirect`; typed text with an idle pane supports `SubmitParked`; working, dialog, wedged, or otherwise unproven states remain `KeepPolling`. These are escalation actions, not delivery proofs. `safe_to_dispatch` is an independent admission fact and cannot be inferred from composer evidence.

### RR-L4-ABSENCE-IS-UNKNOWN

No receipt yet is not a refusal. A single unanswered observation cannot construct `NonDeliveryEscalation::ResendDirect` or `SubmitParked`; escalation requires the receiver state plus composer evidence from the same post-send capture and an upstream receipt assessment. Missing post-send data, a missing pane census, or a tracker failure must remain `Indeterminate`/`KeepPolling`, not become `NoReceipt` or a resend decision.

`PostSendObservation::Missing` and `EmptyPaneList` therefore produce `ReceiptVerdict::Indeterminate`. `NoReceipt` is reserved for a supplied observation that meets a named negative condition such as unchanged idle, no timer reset, stable content, or a wedged unsubmitted prompt. An absent ack is separately handled by `AckVerdict::Missing` or `AckAuthority::NotReadBack`; it is never a receiver refusal.

The current `escalate_non_delivery(post_state, composer)` API accepts only one `PaneState` and one `ComposerEvidence`, so its direct call shape does not carry a two-capture receipt or prove that the observation was post-send. This contract records the required precondition; it does not claim the present function signature mechanically enforces it.

### RR-L5-OBSCURED-UNPROVEN-INHABITED

Obscured and unproven observations are inhabited, named outcomes. Neither may coerce to `Idle`, `ReceiptConfirmed`, `Dead`, or an escalation that assumes non-delivery.

The two-capture `tick-monitor::Liveness` vocabulary names `Obscured` and `Unproven`. In the receiver classifier, the corresponding cases are represented through `PaneState::Dialog`/`ReceiptReason::DialogOpen` for a pane covered by an approval dialog, and `PaneState::Unproven`/`ReceiptReason::ObservationNotWorking` for an unreadable or unrecognised capture. An external `OBSCURED` signal must map to the indeterminate semantic class, not to idle. The current receiver crate does not declare an `Obscured` variant of its own; `tick-monitor` owns that liveness vocabulary and `receiver-receipt` owns the receipt mapping.

Last-line anchoring is mandatory. The status line is selected by `tick-monitor`; a stale task label in scrollback is not state. Timer parsing must use the status-line timer's unit: `1h` is not a turn timer merely because it appears near a turn, and goal-elapsed or spend counters are not receipt evidence. A whole-buffer spinner match is not admissible because it can report `WORKING` and `IDLE` simultaneously from scrollback and the current last line.

## Transition Rules

| Pre-state | Post-state | Positive receipt condition | Otherwise |
|---|---|---|---|
| `Idle` | `Working { timer_secs }` | Fresh timer at most 30 seconds **and** stable content changed. | `Indeterminate` for large timer; `NoReceipt` for unchanged content. |
| `Idle` | `Idle` | Never. | `NoReceipt::IdleUnchanged`. |
| `Working { before }` | `Working { after }` | Timer reset (`after < before`) **and** stable content changed. | `NoReceipt` for no reset or unchanged content. |
| `Working` | `Idle` | Never. | `NoReceipt::PostBecameIdle`. |
| Any | `Dialog` | Never. | `Indeterminate::DialogOpen`. |
| Any | `Wedged` | Never. | `NoReceipt::WedgedUnsubmitted`. |
| Any | `Unproven` | Never. | `Indeterminate::ObservationNotWorking`. |
| Any | absent from non-empty census | Never. | `Dead`. |
| Any | empty census or missing capture | Never. | `Indeterminate`; no death or refusal claim. |

`Dead` is permitted only after a non-empty pane census proves the named pane absent. An empty pane list is not a death certificate.

## Cross-References

- `crates/receiver-receipt/src/lib.rs` — six public types, capture adapter, classifier, and invariant tests
- `crates/receiver-receipt/src/bin/receiver-receipt.rs` — CLI surface
- `crates/tick-monitor/src/lib.rs` — last-line selection, timer parsing, stable hash, `PaneState`, and `Liveness`
- `crates/pane-truth/src/lib.rs` — separate terminal truth authority; no Phase 0 migration
- `crates/omp-types/src/pane_observation.rs` — separate Phase 0 evidence algebra; do not edit in this task
- `docs/contracts/pane_observation_contract.md` — `PO-L1-TWO-CAPTURE-DOMINANCE`
- `crates/ack-spine/src/authorities.rs` — `DeliveryAuthority` consumes `ReceiptVerdict`
- `docs/contracts/ack_spine_contract.md` — transport/delivery/ack separation
- `crates/receiver-receipt/src/lib.rs:464` — idle-to-working receipt test
- `crates/receiver-receipt/src/lib.rs:504` — working timer-reset receipt test
- `crates/receiver-receipt/src/lib.rs:537` — dialog indeterminate test
- `crates/receiver-receipt/src/lib.rs:564` — missing/mismatched observation tests

## Validation

```bash
python3 - <<'PY'
from pathlib import Path
import re

path = Path("docs/contracts/receiver_receipt_contract.md")
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
print(f"RECEIVER_RECEIPT_CONTRACT_BYTES={len(text.encode())}")
print(checks)
if not all(checks.values()):
    raise SystemExit(1)
PY
```

## Non-Coverage

- No transport send, `ntm` result, composer mutation, Enter key, or receiver capture is performed here.
- No stale task label, goal timer, whole-buffer scan, or sender-success boolean is accepted as receipt evidence.
- No raw terminal parser is implemented here. `tick-monitor` owns the last status line, timer units, dialog/state classification, and spinner-stripped hash.
- No `safe_to_dispatch` admission decision is derived from a receiver receipt or composer state.
- No worker comprehension, task completion, bead grade, or tracker acknowledgement is inferred from `ReceiptConfirmed`.
- No current function signature mechanically enforces the 75-second interval before `ReceiptConfirmed` or prevents a direct one-observation escalation call; these are explicit implementation gaps for a later adoption bead.
- No new `Obscured` variant is added to this crate. `tick-monitor::Liveness` owns that vocabulary; receiver-receipt maps obscured/unproven conditions into named indeterminate receipt outcomes.
- No Phase 0 migration changes `pane-truth`, `tick-monitor`, `omp-types`, `ack-spine`, or existing receiver-receipt callers.

## NO-CLAIM

This document pins the evidence boundary but does not repair the existing classifier or claim that all five laws are mechanically enforced today. In particular, the current receiver verdict does not carry the 75-second interval, `NonDeliveryEscalation` can be called without a receipt object, and the source uses `Dialog`/`Unproven` rather than a local `Obscured` variant. The contract does not prove that a packet arrived, was submitted, was understood, or was acknowledged. It does not prove that an empty census means death, that a sender success means delivery, or that a commit or lease release is a receiver receipt.

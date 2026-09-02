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

## Types

Seven public types, named because the bead names them and because a contract that
describes only its IDs cannot be checked against the source. The validator asserts
each `pub enum` below exists and that the count still equals seven.

| Type | Question it answers | Inhabited failure state |
|---|---|---|
| `PanePresence` | Was the named pane in the census? | `PaneListEmpty` — an empty census is not a death certificate. |
| `PostSendObservation` | Was a post-send capture obtained at all? | `Missing` — no capture is not non-delivery. |
| `ReceiptReason` | WHY a verdict is negative or unknown. | Every negative verdict carries one; a bare refusal has no constructor. |
| `ReceiptVerdict` | Did the evidence support delivery? | `Indeterminate { reason }` — distinct from `NoReceipt`. |
| `AckWaitVerdict` | Did an expired ACK wait see busy or unreachable? | `Indeterminate { reason }` — neither a busy finding nor a death claim. |
| `ComposerEvidence` | Is there text in the composer? | `Typed` is arrival, never submission. |
| `NonDeliveryEscalation` | What action does the evidence support? | `KeepPolling` — the default when nothing is proven. |

**Every type has an inhabited unknown.** That is the shared design rule: a real,
distinct condition with no representation gets coerced into a neighbouring value,
and the coercion reads as normal. `AckWaitVerdict` exists because the supervisor
previously reported `ack_readback_missing` for a busy worker and a dead one alike.

## Evidence IDs

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
| `RR-ACK-WAIT-BUSY` | The pane's timer ADVANCED across the observation floor. | That the packet arrived, or that this pane is working on the named bead. |
| `RR-ACK-WAIT-UNREACHABLE` | The pane is wedged, dialog-blocked, quota-halted, idle without an ack, or its timer did not advance across the floor. | Death, or that the packet was never received. |
| `RR-ACK-WAIT-INDETERMINATE` | The window is below the floor, the captures are out of order, the pane ids differ, or the state is unproven. | Busy, unreachable, or delivered. |

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

### RR-L6-ACK-WAIT-DISCRIMINATES

An expired ACK wait must say WHICH of busy or unreachable it observed. `AckWaitVerdict` has three arms and **exactly one may interrupt a human**: `Unreachable`. `BusyStillWorking` answers retry-next-tick; `Indeterminate` answers re-observe-past-the-floor.

The discriminator is **timer advance across `OBSERVATION_WINDOW_MIN_SECS`**, not elapsed time. Measured pane timers sat at 120s, 1320s and 1440s inside a single tool call, so no fixed bound separates a busy worker from a wedged one; widening a window only moves where it guesses wrong. **The window decides when to re-check; it must not decide whether a human is called.** A pane that never acks therefore stays a human debt at every window, and a widening that makes the never-acking case pass is a regression wearing a fix.

`RECEIPT_TIMEOUT` in the supervisor is 30 seconds, which is BELOW this contract's 75-second floor: the old bound could not answer this question even in principle. Landed `d4e8453`; bead `omp-orchestrator-iis6`.

## Cross-Check Against `pane_observation_contract`

Each law of this contract is stated against its counterpart. `pane_observation_contract` owns evidence GRADE; this contract owns delivery VERDICT semantics, so a divergence is a narrowing and never a contradiction.

| This contract | `pane_observation_contract` | Relationship |
|---|---|---|
| `RR-L1-TWO-CAPTURE` | `PO-L1-TWO-CAPTURE-DOMINANCE` | **Stricter.** PO requires a changed timer **or** changed hash for dominance; `ReceiptConfirmed` requires the transition-appropriate timer evidence **and** changed stable content. Receipt confirmation is a subset of two-capture evidence. |
| `RR-L4-ABSENCE-IS-UNKNOWN` | `PO-L2-UNKNOWN-INHABITED` | **Same law, receipt vocabulary.** PO forbids converting unreadable evidence to `Idle`; RR-L4 forbids converting a missing observation into `NoReceipt` or an escalation. Both make absence an inhabited state. |
| `RR-L5-OBSCURED-UNPROVEN-INHABITED` | `PO-L3-LAST-LINE` | **Consumes it.** Last-line anchoring is PO's constructor rule; RR-L5 restates it as a receipt admissibility rule and adds the timer-unit and whole-buffer prohibitions. |
| `RR-L2-FRESH-IDLE-TO-WORKING`, `RR-L6-ACK-WAIT-DISCRIMINATES` | `PO-L4-NO-CONTRADICTION` | **Inherits it.** One `PaneState` is exactly one value, so `Working ∧ Idle` has no constructor and neither a receipt nor an ack-wait verdict can claim both. RR-L6 adds the same property at the verdict layer: exactly one arm owes a human. |
| `RR-L3-COMPOSER-ARRIVAL-NOT-DELIVERY` | `PO-L5-DISPATCH-SEPARATE` | **Same separation, other axis.** PO separates dispatch admissibility from liveness; RR-L3 separates composer arrival from delivery and forbids deriving `safe_to_dispatch` from composer evidence. |

**Unmapped in both directions, stated rather than implied:** PO has no counterpart for `RR-L2`'s freshness bound (`MAX_IDLE_TO_WORKING_TIMER_SECS`), because a 30-second transition bound is a delivery-verdict rule and not an evidence grade. This contract has no counterpart for PO's evidence-ordering section, which it consumes rather than restates.


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

- `crates/receiver-receipt/src/lib.rs` — seven public types, capture adapter, classifier, and invariant tests
- `crates/receiver-receipt/tests/receipt_contract.rs` — the validator for THIS document
- `crates/receiver-receipt/src/bin/receiver-receipt.rs` — CLI surface
- `crates/tick-monitor/src/lib.rs` — last-line selection, timer parsing, stable hash, `PaneState`, and `Liveness`
- `crates/pane-truth/src/lib.rs` — separate terminal truth authority; no Phase 0 migration
- `crates/omp-types/src/pane_observation.rs` — separate Phase 0 evidence algebra; do not edit in this task
- `docs/contracts/pane_observation_contract.md` — `PO-L1`..`PO-L5`, cross-checked above
- `crates/ack-spine/src/authorities.rs` — `DeliveryAuthority` consumes `ReceiptVerdict`
- `docs/contracts/ack_spine_contract.md` — transport/delivery/ack separation
- `crates/omp-orchestrator/src/main.rs` — the supervisor's ACK wait consumes `AckWaitVerdict`

## Validation

One pasteable command. It is a Rust suite, not an interpreter payload: this
repository's one rule is *"No `.sh`. No `.py`."*, the extension gate only sees files,
and the previous version of this section was a `python3` heredoc — the same payload
wearing a shape the gate cannot see.

```bash
cargo test -p receiver-receipt --test receipt_contract
```

**It checks doc-to-SOURCE agreement, not only document shape.** The previous
validator asserted nine structural facts and **passed while this document was
wrong**: it said *"six public types"* when seven existed, because `AckWaitVerdict`
landed the same day in `d4e8453`. A shape-only validator cannot see that — the shape
did not change, only the world did. So the suite asserts every named enum exists in
the source, that the stated count matches `pub enum` declarations, and that the two
quoted constants still hold their quoted values.

It also refused this section's own former contents, and finding that took one more
instance of a familiar defect: `## Non-Coverage` appeared **inside the old python
block** as a regex literal, so a section-slicing check matched the code block
instead of the section. The self-referential checker again — a checker's input
containing text about the thing it checks — this time inside the fix for it.

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

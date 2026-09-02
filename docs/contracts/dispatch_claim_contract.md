# Dispatch Claim Contract

Bead: `omp-orchestrator-omp-coverage-mission-ipg.4` (Phase 1 contract corpus document #12)

## Purpose

This contract defines the claim boundary between a bead and a dispatch: a packet naming an unclaimed bead is a message rather than a dispatch, the durable dispatch ledger is the authority for proving that a dispatch happened, follow-up detection operates only on the projected claimed state, and ownership ends with the work lifecycle rather than with an unrelated reservation clock.

## Contract Artifacts

1. Canonical artifact: `crates/ack-spine/src/spine.rs` — `DispatchIntent`, `PendingDispatch`, and the durable dispatch ledger boundary.
2. Runner: the single pasteable command in `## Validation` below.
3. INVARIANT SUITE: `crates/ack-spine/tests/spine.rs`, `crates/ack-spine/tests/followup.rs`, and `crates/ack-spine/tests/authorities.rs`.

## Dispatch Claim Model

The lifecycle is explicit and ordered:

```text
file -> claim -> dispatch -> observe -> verify -> close
              ^
       required projection
```

| Value | Stable ID | Meaning |
|---|---|---|
| `ClaimedBead` | `DCL-V-CLAIMED` | The tracker says `status=in_progress` and the bead is assigned to the receiving agent. |
| `DispatchIntent` | `DCL-V-INTENT` | A claimed bead, target pane/agent, and session identity prepared for one dispatch attempt. |
| `Dispatched` | `DCL-V-DISPATCHED` | The dispatch ledger durably records that the intent was sent or attempted. It is not delivery or acknowledgement. |
| `FollowUpProjection` | `DCL-V-FOLLOWUP` | The claimed, in-progress tracker projection that permits `classify_followup` to detect silence. |
| `OwnershipEnd` | `DCL-V-OWNERSHIP-END` | The work closes, is cancelled, is reassigned, or is otherwise resolved; the claim must end with that lifecycle event. |

A bead's `open` and unassigned state is not itself proof of a skipped claim. It may be ordinary unstarted work. The positive signal for a skipped claim is a durable dispatch-ledger row paired with an unclaimed tracker snapshot.

## Laws

Each law names the existing test that exercises the nearest current boundary. These tests establish the current projection and ledger behavior; where the implementation remains permissive, the gap is recorded rather than presented as enforced.

- **DCL-L1-CLAIM-BEFORE-DISPATCH** — a packet naming a bead is a dispatch only when the bead is `in_progress` and assigned to the receiving agent. `DispatchIntent` and `PendingDispatch` must not be constructible from an unclaimed record. *Test:* `crates/ack-spine/tests/spine.rs::one_row_per_step_and_three_authorities_are_independent` exercises the positive claimed dispatch lifecycle; constructor refusal for an unclaimed record is an implementation gap named in `Non-Coverage`.
- **DCL-L2-STATUS-PROJECTION** — the bead status is the tracker projection of the claim that preceded dispatch. Skipping the claim leaves the tracker open/unassigned while the packet is carried, so the queue can continue offering the bead. *Test:* `crates/ack-spine/tests/spine.rs::cancellation_after_send_persists_recoverable_marker_and_blocks_retry` proves that the dispatch ledger persists a durable lifecycle projection rather than relying on pane state.
- **DCL-L3-FOLLOWUP-BLIND-SPOT** — `classify_followup` can detect dispatched-then-silent only when the assigned + `in_progress` projection exists and no comment arrived. A dispatched-but-unclaimed bead produces no follow-up signal; the ledger must classify that state separately as `DISPATCHED_UNCLAIMED`. *Test:* `crates/ack-spine/tests/followup.rs::dispatched_silent_past_deadline_is_typed` proves the projected claimed path; no current test claims to detect an unprojected dispatch.
- **DCL-L4-LEDGER-AUTHORITY** — the dispatch ledger, not an isolated bead read, establishes that a dispatch occurred. `open` plus unassigned is not evidence of a skipped claim without the ledger's dispatch row and captured claim state. *Tests:* `crates/ack-spine/tests/spine.rs::one_row_per_step_and_three_authorities_are_independent` and `crates/ack-spine/tests/spine.rs::cancellation_after_send_persists_recoverable_marker_and_blocks_retry`.
- **DCL-L5-CLAIM-DIES-WITH-WORK** — ownership ends with close, cancellation, reassignment, or explicit resolution of the work. A reservation TTL is a crash-recovery fallback, not proof that work ended. *Tests:* `crates/ack-spine/tests/spine.rs::cancellation_after_send_persists_recoverable_marker_and_blocks_retry` and `crates/ack-spine/tests/followup.rs::reassigned_bead_is_not_silent`.

## Authority Agreement with `ack_spine_contract.md`

**Agree.** `docs/contracts/ack_spine_contract.md` already separates transport, receiver delivery, and tracker acknowledgement (`AS-T-TRANSPORT`, `AS-D-DELIVERY`, `AS-A-ACK`) and states `AS-L5-CLAIM-BEFORE-DISPATCH`: a claimed bead is `in_progress` and assigned to the receiving agent. This contract adopts that boundary without weakening it. We also agree that `PendingDispatchStore` is the durable uncertainty authority and that a cleared marker is not delivery or worker acknowledgement.

**Extend.** This contract specializes `AS-L5` into the dispatch constructor and ledger boundary: `DispatchIntent` must consume claimed evidence, the ledger must retain the claim snapshot alongside the dispatch attempt, and an unclaimed dispatch is a distinct ledger state rather than a silent follow-up miss. It also extends the ownership discussion in `ack_spine_contract.md`: a file-reservation claim may bind release to a path-scoped commit observed, with TTL as crash fallback; a dispatch claim may not bind only to a commit, because a commit proves code landed, not that the receiving agent accepted the bead. Dispatch ownership binds to tracker lifecycle plus ledger evidence.

**Disagree.** No contradiction was found between these contracts. The only apparent tension — “the ledger is authority” versus “the bead claim is required” — is resolved by scope: the bead supplies the claim projection, while the dispatch ledger supplies the evidence that a dispatch happened. Neither source alone proves the full lifecycle.

## Ledger Record

A conforming dispatch ledger row carries, at minimum:

- bead identity, target agent/pane, session identity, and dispatch attempt identity;
- the tracker claim snapshot: status, assignee, and capture time;
- the dispatch timestamp and transport attempt result;
- a state distinguishable as `CLAIMED_DISPATCHED` or `DISPATCHED_UNCLAIMED`;
- follow-up/verification outcome and the lifecycle event that ends ownership.

The row is append-only evidence for the attempt. A later `br show` may report the bead's current state, but it cannot retroactively establish that an earlier unclaimed packet had been claimed.

## Claim Lifetime and Commit Binding

A claim must name the thing that ends it. For a bead dispatch, the owner is the work lifecycle: close, cancellation, reassignment, or explicit operator resolution. For a file reservation, the preferred release receipt is a path-scoped commit observed after the work; the TTL exists only to recover from a dead process or lost operator. A commit is not a transport receipt, delivery receipt, or worker acknowledgement, and a TTL expiry is not a work-completion receipt.

The measured failure shape is therefore explicit: a reservation can expire at a clock time after the protected work has already finished, blocking a live worker. Binding release to an observed commit reduces that window, but only a reservation owner or an explicit lifecycle controller can release the lock; this contract does not claim that Agent Mail or git currently enforces the whole relationship.

## Non-Coverage

- No migration or hardening of `DispatchIntent::new`, `PendingDispatch`, `classify_followup`, or any caller.
- No claim that current constructors refuse unclaimed beads; `ack_spine_contract.md` records that the public constructor still accepts raw identifiers and that this is an implementation gap.
- No new `DISPATCHED_UNCLAIMED` enum or ledger field in this pass; the state is specified for the later implementation bead.
- No change to `br`, queue selection, pane observation, transport delivery, acknowledgement read-back, or worker completion.
- No proof that a commit was observed by Agent Mail, or that a TTL is reconciled automatically with work completion.
- No claim that an `open`/unassigned bead is a skipped claim without a dispatch-ledger row.
- No claim that the existing follow-up detector can see an unprojected dispatch; L3 records that blind spot.

## Validation

```bash
RCH_ENABLED=false CARGO_MINT_MIN_CONTAINER_PCT=0 cargo test -p ack-spine --tests -- --nocapture
```

The command runs the existing ack-spine ledger, authority-separation, and follow-up invariant suites without starting the full workspace suite.

## Cross-References

- `crates/ack-spine/src/spine.rs` — `DispatchIntent`, `PendingDispatch`, and durable ledger state.
- `crates/ack-spine/src/followup.rs` — `classify_followup` and the assigned/in-progress silence predicate.
- `crates/ack-spine/src/ledger.rs` — typed dispatch lifecycle rows and ledger authority.
- `crates/ack-spine/tests/spine.rs` — ledger persistence, cancellation, retry blocking, and authority lifecycle tests.
- `crates/ack-spine/tests/followup.rs` — projected dispatched-then-silent and reassignment tests.
- `crates/ack-spine/tests/authorities.rs` — transport, delivery, and acknowledgement separation tests.
- `docs/contracts/ack_spine_contract.md` — upstream authority separation, claim-before-dispatch, and commit/TTL ownership discussion.
- `crates/fast-dispatch/src/lib.rs` — current admission and dispatch caller boundary.
- `crates/tick-dispatch/src/lib.rs` — current operational dispatch decision boundary.
- `crates/verify-dispatch/src/lib.rs` — current verification boundary.
- `docs/plans/plan_to_write_the_document_corpus.md` — contract corpus manifest and Phase 1 order.
- `docs/contracts/asupersync_process_grade.md` — single-document pass bar.
- `/Users/josh/.claude/skills/project-startup/assets/contract-template.md` — contract shape.

## NO-CLAIM

A written rule refuses no packet until the dispatch path enforces it. This contract names the required claim, projection, ledger, follow-up, and ownership boundaries; it does not convert a single caller, make `DispatchIntent` unconstructible from unclaimed state, or prove that any current packet obeys the sequence.

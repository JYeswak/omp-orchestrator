# Oracle Comparison Contract

Bead: `omp-orchestrator-oe2`

## Purpose

This contract defines how two observations of the same fleet surface are compared before a mutating action. A dual read is necessary but not sufficient: the combinator must keep `UNKNOWN` inhabited, make an empty or unreadable arm an error, make disagreement an error, and preserve the conservative requirement that dispatch, kill, or reap proceeds only when both usable surfaces agree. It exists because `refill-idle-panes` intersected tmux's four newly-idle panes with an ntm activity projection reporting zero panes, interpreted the empty intersection as “nothing to do,” and exited zero; a missing projection was collapsed into a negative assertion instead of a typed nonzero refusal.

## Contract Artifacts

1. Canonical comparator: `crates/oracle-compare/src/lib.rs` (`OracleCompareRule`, `OracleCompareRules`, `compare_counts`, and `compare_sets`).
2. Consumer boundary: `crates/fleet-reconcile/src/lib.rs` (NTM/tmux comparison and explicit disagreement verdicts).
3. INVARIANT SUITE: `crates/oracle-compare/src/lib.rs` `#[cfg(test)] mod tests`, plus `crates/fleet-reconcile/tests/differential.rs` and `crates/pane-oracle-diff/tests/planted_known_bads.rs`.

The canonical source already names `OracleCompareRule::EmptyOracleIsError`; this contract cites that rule rather than re-deriving a second empty-oracle policy.

## Oracle Comparison Model

Every comparison state gets a stable ID.

| Value | Property | Description |
|---|---|---|
| `OC-OBSERVATION-ORACLE` | independent | The expected/reference surface, such as tmux or a preserved fixture. |
| `OC-OBSERVATION-PRODUCT` | compared | The projection or product output being checked against the oracle. |
| `OC-AGREE` | success | Both usable arms describe the same non-unknown value. |
| `OC-DISAGREE` | restrictive-error | Usable arms differ; no side wins by preference. |
| `OC-UNKNOWN-EMPTY` | restrictive-error | An oracle arm is empty where absence is not itself measured. |
| `OC-UNKNOWN-UNREADABLE` | restrictive-error | An arm could not be read or parsed. |
| `OC-EMPTY-PRODUCT` | conservative-error | The product arm is empty while the oracle has a nonempty value. |
| `OC-ACTION-ADMITTED` | guarded-success | A mutating action may proceed only after the required agreement predicate passes. |

### Properties

- **OC-P1-EMPTY**: an empty oracle arm is `ERROR`, never agreement.
- **OC-P2-UNREADABLE**: an unreadable or unparseable arm is `ERROR`, never agreement.
- **OC-P3-DISAGREEMENT**: different usable values are `ERROR`, never a tie-break.
- **OC-P4-CONSERVATIVE-ACTION**: requiring both surfaces to agree before mutation protects a working pane from one stale read; it does not authorize an empty intersection.
- **OC-P5-TYPED-REFUSAL**: every refusal is a typed nonzero outcome, not a reassuring zero log line.
- **OC-P6-UNKNOWN-INHABITED**: absence remains `UNKNOWN` until a separate rule proves actual absence.

## Laws

- **L1-EMPTY-ORACLE-IS-ERROR** — `OracleCompareRule::EmptyOracleIsError` maps an empty oracle arm to `Unmeasurable`, not `Agree`. *Test:* `crates/oracle-compare/src/lib.rs::tests::empty_oracle_set_is_unmeasurable`.
- **L2-UNREADABLE-IS-ERROR** — either unreadable arm maps to Unmeasurable; no unreadable input can produce agreement. *Test:* crates/fleet-reconcile/tests/differential.rs::rust_matches_shell_on_nonempty_fixture_set (the unparseable fixture).
- **L3-DISAGREEMENT-IS-ERROR** — unequal usable arms map to `Disagree`; no comparator silently prefers oracle or product. *Test:* `crates/oracle-compare/src/lib.rs::tests::disagree_is_finding`.
- **L4-CONSERVATIVE-MUTATION** — both surfaces must agree before dispatch, kill, or reap; an empty or unknown arm instead refuses loudly because “cannot tell” is safer than “nothing to do.” *Test:* `crates/fleet-reconcile/tests/differential.rs::comparator_sees_manufactured_disagreement`.
- **L5-TYPED-NONZERO** — Disagree and Unmeasurable carry nonzero exit codes (1 and 2 respectively), while only Agree carries zero. *Test target:* crates/oracle-compare/src/lib.rs::OracleCompareVerdict::exit_code; dedicated assertions are an oe2 gate acceptance item, not claimed by this contract-only pass.

The core algebra is the distinction between these two combinators:

| combinator | missing/empty arm | result |
|---|---|---|
| intersection for a mutating selector | absent from the intersection | “nothing to do,” often zero — **wrong when the arm is unknown** |
| differential comparison | absent/unreadable arm | `UNKNOWN`/`ERROR`, nonzero — **the required behavior** |

The conservative direction is preserved on both sides: do not mutate a working pane unless the independent surfaces agree, and do not infer no work from missing data. A dual read is therefore a precondition, not an oracle by itself.

## Authority Boundary

`ground_truth_contract.md` owns what each surface is entitled to assert: tmux answers pane existence and geometry, while ntm returns a projection that may be stale or empty-success. `pane_observation_contract.md` owns the observation algebra, including **L2: `Unknown` is a first-class value and NEVER coerces to `Idle`**. This contract owns only the comparison/combinator step between those observations and the action gate that consumes its verdict.

The measured failure was not lack of two reads. At cron times 8, 28, and 48, tmux exposed four newly-idle panes while `ntm --robot-activity=omp-orchestrator` exposed zero. Intersection treated that absence as no idle intersection and exited 0. Differential comparison must instead preserve the missing arm as `Unknown`, emit a typed nonzero refusal, and retain both raw observations for diagnosis.

## Rules

- **OC-R1-SAME-INPUTS**: each arm describes the same session, time window, and subject set; a comparison of different scopes is `Unknown`, not agreement.
- **OC-R2-EMPTY-FIRST**: evaluate empty and unreadable conditions before equality; an empty/empty pair is not automatically equal.
- **OC-R3-FAIL-CLOSED-MUTATION**: dispatch, kill, and reap require `Agree` over nonempty, readable inputs. `Disagree`, `Unmeasurable`, and unknown scope are nonzero refusals.
- **OC-R4-NO-TIE-BREAK**: never silently prefer tmux, ntm, a cached label, or the first readable arm when the independent arm disagrees.
- **OC-R5-EVIDENCE**: a refusal records oracle/product identity, command, exit code, scope, raw payload path, and comparison reason; a log message without a typed outcome is not human-reaching evidence.
- **OC-R6-FRESH-NOT-SUFFICIENT**: a freshness bit may be reported but never bypasses empty-oracle or disagreement handling.

## Validation

```bash
cargo test -p oracle-compare --lib --offline -- --nocapture
```

## Cross-References

- `crates/oracle-compare/src/lib.rs:17-23` — comparison rules, including `OracleCompareRule::EmptyOracleIsError`
- `crates/oracle-compare/src/lib.rs:79-105` — typed comparison verdicts and exit codes
- `crates/oracle-compare/src/lib.rs:112-195` — count and set combinators
- `crates/oracle-compare/src/lib.rs:363-470` — invariant tests for disagreement, unreadable, empty-oracle, and mutation behavior
- `crates/fleet-reconcile/src/lib.rs` — NTM/tmux ground-truth comparator
- `crates/fleet-reconcile/tests/differential.rs` — independent shell/Rust differential oracle
- `crates/pane-oracle-diff/tests/planted_known_bads.rs` — planted empty/undercount oracle failures
- `docs/contracts/ground_truth_contract.md` — surface authority: tmux direct oracle versus ntm projection
- `docs/contracts/pane_observation_contract.md` — `Unknown` is first-class and never coerces to `Idle`
- `docs/contracts/lifecycle_contract.md` — restrictive lifecycle terminals, outside this comparator boundary
- `~/.claude/references/claude-md-ntm-defects.md` — measured stale projection, fresh-zero, reliable-surface, and serial-probe defects
- `docs/plans/plan_to_pin_the_orchestrator_type_algebra.md` — contract before type and bead ordering

## Non-Coverage

- This contract does not define tmux or ntm authority; it consumes the boundary from `ground_truth_contract.md`.
- It does not classify pane liveness, readiness, geometry, lifecycle terminals, or process cancellation.
- It does not choose a retry interval, repair a stale projection, or mutate a pane/session.
- It does not prove that a caller routes every dispatch, kill, or reap through the comparator; consumer wiring is a separate gate.
- It does not turn an empty product into a failure when the oracle itself is legitimately and independently known to be empty; that requires an explicit, scoped presence/absence proof.

## NO-CLAIM

A passing comparator test proves only the named fixture laws and the comparator's typed outcomes. It does not prove the live NTM projection is complete, tmux is reachable, a pane is idle, or a mutating caller actually consumed the refusal. The contract prevents unknown from becoming an accidental negative assertion; it does not make disagreement impossible or guarantee that every runtime path is wired through this comparator.

# Verification Contract

Bead: `omp-orchestrator-verification-contract-yxbs`

## Purpose

Define verification as a fresh, non-vacuous read of durable work state: bead status is the completion authority, tests must be re-run against the current production path, ignored or skipped legs provide no credit, and the grader must be independent of the implementer. This contract separates a worker's report, a verifier's observation, a test result, and a grade so none can silently substitute for another.

## Contract Artifacts

1. Canonical verifier: `crates/verify-dispatch/src/lib.rs` — `bead_status_via_br`, `parse_br_status`, and `run_live`
2. CLI surface: `crates/verify-dispatch/src/main.rs` — live output and mutation-rule controls
3. Invariant suite: `crates/verify-dispatch/src/lib.rs` `#[cfg(test)] mod tests` and `crates/verify-dispatch/tests/differential.rs`
4. Real-data ledger leg: `crates/no-shell-gate/tests/findings_ledger.rs::real_findings_ledger_is_strictly_valid`
5. Grading authority: `AGENTS.md` grading gate and the bead status/read-back workflow

## Verification Model

| Value / ID | Authority | Meaning | Not enough for |
|---|---|---|---|
| `VC-WORKER-CLAIM` | Worker or pane | A self-report of completion, understanding, or success. | Verification or grade. |
| `VC-BEAD-STATUS` | `br show <bead> --json` | Durable tracker status read-back, the only completion state this verifier reads. | Proof that the worker's report was true or that the bead was independently graded. |
| `VC-TEST-RESULT` | Fresh test process | A test existed, ran, exited 0, and asserted against the production path. | Proof when it was skipped, ignored, fixture-only, or stale. |
| `VC-VERIFIED` | `verify-dispatch` | Every named bead for a current dispatch set read back as `closed`. | Delivery, comprehension, or independent grading. |
| `VC-NO-EVIDENCE` | `verify-dispatch` | One or more named beads is not read back as closed, or the set is incomplete. | A failure verdict about why the work is incomplete. |
| `VC-IGNORED-LEG` | Test harness | A leg marked `#[ignore]` or omitted by default. | A passing leg; it is unrun evidence. |
| `VC-GRADER-INDEPENDENT` | Process gate | A non-implementer re-ran the acceptance criteria and recorded the evidence. | Self-certification by the author. |
| `VC-LEGACY-DISPATCH` | Ledger policy | A dispatch without bead IDs cannot be reconstructed from the overwritten packet file. | A guessed status or retroactive completion claim. |

`verify-dispatch` is a reporter, not a gate: the live binary intentionally returns exit 0 for both `VERIFIED` and `NO EVIDENCE`. The semantic stdout row and its cited evidence must therefore be inspected; exit 0 alone is never `VC-VERIFIED`.

## Laws

### VC-L1-BEAD-STATUS-ONLY

Verification reads bead status only, never a pane's self-report, task label, idle flag, composer text, or worker narrative. `bead_status_via_br` runs `br show <bead> --json` and `parse_br_status` extracts the tracker `status`; `run` counts only `closed` statuses. A worker report is `VC-WORKER-CLAIM`, not evidence that the bead is closed.

A pane may say DONE while the bead remains open, or the bead may be closed while the pane is dead. Those are different facts. The verifier does not infer one from the other. The receiver, transport, and pane-observation contracts remain separate authorities and cannot be substituted for tracker status.

### VC-L2-RERUN-CURRENT

Verification must re-run the relevant command against the current checkout, current dependency/artifact identity, and current external data. A test or CI result from yesterday is historical context, not current evidence. The verifier must record the exact command, source revision, test target, and exit result; reading an old green log is inadmissible.

`VerifyDispatchConfig` derives a current time and applies the configured dispatch window, but that timestamp filter is not a substitute for re-running the verifier or for proving the binary's source identity. A report copied from a prior run remains stale even if its text says `VERIFIED`.

### VC-L3-NONVACUOUS-TEST

A test passes meaningfully only when all three conditions hold:

1. **Exists:** the named test target and test function are present in the current checkout.
2. **Exits 0:** the test process genuinely completes successfully.
3. **Asserts non-trivially:** the assertion exercises the production function, binary, gate, or real data path and would fail for the named defect.

A fixture-only test, a test that only asserts setup, an output that was never checked, a zero-case comparison, and a skipped external oracle fail this law. `verify-dispatch`'s closed/open/partial tests assert the production `run` output; its differential suite first manufactures a disagreement and then compares a non-empty case set. The differential helper explicitly prints `0 cases compared ... NOT a passing differential` when its oracle is unavailable.

`VC-TEST-RESULT` is stronger than a process exit code: `verify-dispatch` may exit 0 while reporting `NO EVIDENCE`, because it reports rather than gates. The result must carry the semantic assertion and production-path target.

### VC-L4-IGNORED-IS-UNRUN

An `#[ignore]`d leg is not a passing leg. `findings_ledger::real_findings_ledger_is_strictly_valid` is the only leg that validates the real `docs/plan/FINDINGS.jsonl` rather than a synthetic fixture. Before `4443c43` it was `#[ignore]`d and `--include-ignored` had zero in-tree callers; that historical shape let default `cargo test` report green while production data was unchecked. The current source is un-ignored, but the contract retains the violation because the same omission can recur if the real-ledger leg is skipped again.

The real-ledger leg receives explicit credit only when the current command includes `--include-ignored`, the test exists, the process exits 0, and the test asserts the real ledger. Fixture tests remain useful for branch behavior but cannot substitute for that leg. `VC-IGNORED-LEG` is a named absence of evidence, never a green result.

### VC-L5-INDEPENDENT-GRADER

The grader must not be the implementer. An implementation commit, worker report, bead close reason, or test result authored by the same agent is a claim awaiting independent re-execution. A different agent must re-run the bead's acceptance criteria from the current checkout, inspect the actual result, and record the command, revision, and evidence before closing the bead.

The verifier reports tracker status; it does not turn a worker's prose into a grade. Independent grading is a process authority outside `verify-dispatch`'s Rust types. If the identity of the implementer or grader is unavailable, the grade is unknown, not approved.

## Evidence Ordering and Failure Semantics

The evidence order is deliberately asymmetric:

```text
worker report  -> claim only
fresh test     -> test evidence, if non-vacuous
br status      -> completion projection
independent re-run -> grade
```

`VC-VERIFIED` means only that every named bead in a current, bead-ID-bearing dispatch set read back as `closed`. `VC-NO-EVIDENCE` means the set is not fully closed; it does not distinguish in-flight work, packet loss, partial completion, or an incorrect bead ID. `VC-LEGACY-DISPATCH` is reported rather than reconstructed. A missing ledger, empty dispatch window, unreadable tracker, or malformed input is a named no-evidence/error condition, not a silent pass.

## VIOLATION — where shipped code contradicts this law

- **Declared:** `VC-L4-IGNORED-IS-UNRUN` requires the real findings ledger to run for a passing claim. **Historical shipped:** before `4443c43`, `crates/no-shell-gate/tests/findings_ledger.rs:775` documented and applied `#[ignore]` to `real_findings_ledger_is_strictly_valid`, while `--include-ignored` had zero in-tree callers. **Consequence:** default cargo test reported green while `FINDINGS.jsonl` was unchecked, the measured BUILT ≠ WIRED failure. **Current status:** `4443c43` removed the ignore; the historical violation remains the reason this law and explicit `--include-ignored` validation are required.
- **Declared:** `VC-L3-NONVACUOUS-TEST` distinguishes semantic verification from exit status. **Shipped:** `crates/verify-dispatch/src/main.rs:3-5` documents that the live path always exits 0, and `crates/verify-dispatch/src/lib.rs:474-483` returns code 0 after reporting both verified and no-evidence rows. **Consequence:** a caller that checks only exit 0 can certify an incomplete dispatch.
- **Declared:** `VC-L5-INDEPENDENT-GRADER` requires grader identity and a non-implementer re-run. **Shipped:** `crates/verify-dispatch/src/main.rs:52-55` prints the run output and returns its code without carrying implementer/grader identity. **Consequence:** the binary cannot enforce the independent-grader process gate; the identity check remains external and must be recorded by the bead workflow.
## Non-Coverage

- No pane self-report, task label, composer state, transport result, or receiver receipt is accepted as completion evidence.
- No verifier run proves packet delivery, worker comprehension, or task quality; bead status is the completion projection, not a semantic proof of work.
- No old CI log, prior artifact, copied stdout, or historical bead comment satisfies `VC-L2-RERUN-CURRENT`.
- No ignored, skipped, zero-case, fixture-only, or setup-only test satisfies `VC-L3-NONVACUOUS` or `VC-L4-IGNORED-IS-UNRUN`.
- No code change is made here to enforce execution of the real-ledger leg, prevent a future `#[ignore]`, add an in-tree `--include-ignored` caller, or enforce grader identity. The historical ignore was removed by `4443c43`; this contract records the failure mode and its required evidence boundary.
- No full workspace suite is part of this contract; the validation command targets the real findings-ledger leg only.
- No `VC-VERIFIED` claim is made for an unassigned, legacy, malformed, unreadable, or partially closed dispatch set.

## Validation

```bash
RCH_ENABLED=0 PATH="$HOME/.rustup/toolchains/nightly-2026-08-23-aarch64-apple-darwin/bin:$PATH" "$HOME/.rustup/toolchains/nightly-2026-08-23-aarch64-apple-darwin/bin/cargo" test -p no-shell-gate --test findings_ledger -- --include-ignored --nocapture --test-threads=1
```

## Cross-References

- `crates/verify-dispatch/src/lib.rs` — bead-status reader, current-window filter, semantic verifier, and unit tests
- `crates/verify-dispatch/src/main.rs` — reporter exit semantics and mutation controls
- `crates/verify-dispatch/tests/differential.rs` — non-empty Rust/Python oracle comparison and manufactured-disagreement leg
- `crates/no-shell-gate/tests/findings_ledger.rs:773-791` — historical ignored real-ledger violation and current real-ledger leg
- `crates/no-shell-gate/tests/findings_ledger.rs:591-753` — fixture and reconciliation coverage
- `AGENTS.md:932-938` — measured ignored-leg and zero-caller finding
- `docs/contracts/dispatch_claim_contract.md` — claim-before-dispatch and lifecycle ownership boundary
- `docs/contracts/ack_spine_contract.md` — transport/delivery/ack authorities remain distinct
- `docs/contracts/receiver_receipt_contract.md` — receiver evidence is not completion or acknowledgement
- `docs/plans/plan_to_write_the_document_corpus.md` — Phase 2 manifest entry 20

## NO-CLAIM

This contract defines the verification evidence boundary but does not make callers obey it. It does not guarantee that the real-ledger leg is run by every future workflow, create an in-tree `--include-ignored` caller, prove current source identity for copied reports, or enforce non-implementer grader identity in Rust. A green test process is not meaningful without existence, exit 0, and a non-trivial production-path assertion. A `VC-VERIFIED` row still does not prove delivery, comprehension, quality, or completion beyond the bead status authority it explicitly reads.

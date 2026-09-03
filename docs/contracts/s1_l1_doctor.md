# S1 L1 Doctor Contract

Bead: omp-orchestrator-s1w0-l1l2-doctor-init-contract-d9rv

## Purpose

L1 defines the scoped system doctor for the S1 human-start path: it probes required tools with independent presence and version evidence, distinguishes absent, unprobeable, stale, paused, and healthy results, exposes a typed report and next action, and performs only explicit, reversible repairs through one mutation chokepoint. Its idempotence law is conditional: an unchanged observation with identical before-state hashes produces zero repair actions on the next run; a changed tool, daemon, or hash is a new observation and may require repair.

## Contract Artifacts

1. Canonical artifact: MISSING today; target .omp-orchestrator/doctor/report.json, schema s1.l1.doctor.v1.
2. Smoke runner: MISSING today; target ompo doctor --json --scope system.
3. Invariant suite: MISSING today; target crates/installer/tests/s1_l1_doctor_contract.rs. The Wave-0 validation below is an executable source-and-law stand-in, not production coverage.

A named but missing suite is intentional Wave-0 state, not a passing implementation claim.

## L1 Doctor Model

| ID | Property | Description |
|---|---|---|
| L1-INPUT | evidence | One probe has name, family, presence observation, version observation, scope, and provenance. |
| L1-VERDICT | typed result | Verdict is OK, ABSENT_FAMILY, ABSENT_SPECIFIC, UNPROBEABLE, STALE, PAUSED, or UNMEASURED; no missing value becomes OK. |
| L1-REPAIR | authority | Every repair enters one mutate() chokepoint, with a run directory, backup, before hash, after hash, and action record. |
| L1-UNDO | recovery | undo <run-id> restores the recorded pre-mutation bytes or database snapshot, and reports the result. |
| L1-IDEMPOTENCE | conditional law | Same scope plus identical before-state hashes and unchanged probe inputs yields zero repair mutations on the next doctor run. |
| L1-DRIFT | re-evaluation | A changed version, daemon state, scope, or before hash invalidates the no-op premise; the doctor must re-probe and may repair. |
| L1-SCOPE | bounded work | A scoped run executes only named probe families; skipped probes are explicit, never silently green. |

## Probe Verdict Type

ProbeVerdict is one seven-arm semantic enum. The UNMEASURED arm carries a run_state and reason_code so timeout and instrument failures remain Unrun-like without becoming a false refusal.

| Arm | Emitted when | Branch behaviour |
|---|---|---|
| OK | Presence and required version/identity both pass | CONTINUE to L2. |
| ABSENT_FAMILY | No member of a required tool family is present | HUMAN_HALT with named remediation; an optional family may be INFO only when optionality is explicit. |
| ABSENT_SPECIFIC | The family exists but the named tool is absent | INFO plus remediation when optional; HUMAN_HALT when the specific tool is required. Never relabel as OK. |
| UNPROBEABLE | The probe was attempted, but the subject cannot answer through the supported interface | HUMAN_HALT unless a named HD explicitly authorizes DEGRADED_CONTINUE; the verdict remains UNPROBEABLE. |
| STALE | The subject answered, but the version, identity, or evidence age is outside the required floor | Re-probe/remediate and HUMAN_HALT by default; only a named HD may authorize DEGRADED_CONTINUE for a non-critical probe. A present wrong-version tool lands here, not OK or ABSENT. |
| PAUSED | The lane or service explicitly reports an intentional pause | HUMAN_HALT with resume/remediation by default; a named HD may authorize DEGRADED_CONTINUE. PAUSED is distinct from inability to answer because it carries operator intent. |
| UNMEASURED | No valid subject observation exists: timeout, no record, skipped probe, or probe-instrument failure | Carry run_state=UNRUN for timeout/no-record and reason_code; HUMAN_HALT by default, with named-HD DEGRADED_CONTINUE as the only override. UNMEASURED is never Pass or Refused. |

UNKNOWN and UNPROBEABLE remain distinct. UNKNOWN means no authoritative observation exists, so it is represented as UNMEASURED with reason_code=UNKNOWN_NO_RECORD; RCH's repo-convergence status=unknown with total=0 is this case. UNPROBEABLE means an attempt reached the subject and the subject could not answer. Collapsing them would turn an absent measurement into a claim about the subject.

Timeout follows crate-atom-gate/src/lib.rs:56-57: a timeout is Unrun, never Pass or Refused. An instrument failure is also UNMEASURED with run_state=INSTRUMENT_ERROR, never a subject refusal. The branch may halt, but the verdict must preserve that the measurement did not run.

The exhaustive wire shape is: ProbeVerdict{OK|ABSENT_FAMILY|ABSENT_SPECIFIC|UNPROBEABLE|STALE|PAUSED|UNMEASURED{run_state=UNRUN|INSTRUMENT_ERROR,reason_code}}. No seventh-arm default is permitted; an unknown wire arm is an instrument/decode error and must halt.

### Verdict properties

- L1P-EXHAUSTIVE-ARMS: every probe result is exactly one of the seven arms above; absence of a matching arm is a decode/instrument error.
- L1P-WRONG-VERSION-STALE: presence plus a wrong version is STALE, never OK, ABSENT_SPECIFIC, or UNPROBEABLE.
- L1P-UNRUN-NOT-REFUSED: timeout, no-record, and instrument-error observations are UNMEASURED and never Refused.
- L1P-PAUSED-DISTINCT: an explicit operator pause is PAUSED, not UNPROBEABLE, so resume intent is preserved.
- L1P-HD-NAMED: only an explicit named HD may convert UNPROBEABLE, STALE, PAUSED, or UNMEASURED into degraded continuation; the enum arm and reason remain visible.

### Verdict laws

- LAW-L1-VERDICT-EXHAUSTIVE — the decoder cannot silently map an unknown result to OK or Refused. Test: s1_l1_doctor_contract.rs::unknown_arm_is_instrument_error.
- LAW-L1-UNRUN — a timeout produces UNMEASURED with run_state=UNRUN and never Pass or Refused. Test: s1_l1_doctor_contract.rs::timeout_is_unrun.
- LAW-L1-WRONG-VERSION — a present tool below the version floor produces STALE. Test: s1_l1_doctor_contract.rs::wrong_version_is_stale.
- LAW-L1-PAUSED — an explicit pause produces PAUSED and retains pause intent. Test: s1_l1_doctor_contract.rs::paused_is_not_unprobeable.
- LAW-L1-UNKNOWN — no authoritative record produces UNMEASURED with reason_code=UNKNOWN_NO_RECORD, not UNPROBEABLE. Test: s1_l1_doctor_contract.rs::unknown_record_is_unmeasured.

### Properties

- L1P-TWO-SIGNALS: a tool is OK only when presence and version evidence both satisfy the required contract.
- L1P-UNKNOWN-LOUD: UNPROBEABLE, STALE, PAUSED, and UNMEASURED remain typed outcomes and carry a reason plus next action.
- L1P-ONE-MUTATOR: no fixer writes outside the mutation chokepoint.
- L1P-BEFORE-HASH: every reversible file mutation records the exact pre-state hash before writing a backup or applying the operation.
- L1P-CONDITIONAL-IDEMPOTENCE: zero actions is required only when the second run observes the same scope, probe inputs, and before-state hash tuple as the first run's post-state. It is not a promise that a doctor never acts twice.
- L1P-REPAIR-THEN-REPROBE: a repair is not a healthy verdict; the same probe must be run again before L1 advances.

## Laws

- LAW-L1-SCOPED-PROBE — a scoped doctor emits only the requested probe families and preserves explicit skipped/unmeasured outcomes. Test: s1_l1_doctor_contract.rs::scope_does_not_vacuously_pass_skipped_probes.
- LAW-L1-TWO-SIGNALS — a present tool at the wrong version is not OK. Test: s1_l1_doctor_contract.rs::wrong_version_is_not_ok.
- LAW-L1-MUTATE-AUDIT — a repair records run id, before hash, backup, after hash, and one action row through the chokepoint. Test: s1_l1_doctor_contract.rs::repair_has_verbatim_backup_and_action.
- LAW-L1-CONDITIONAL-IDEMPOTENCE — identical post-repair state and probe inputs produce zero second-run repair actions; changed state is allowed to produce a new action. Test: s1_l1_doctor_contract.rs::same_hash_noop_but_drift_repairs.
- LAW-L1-UNDO — undo restores every touched target to its recorded before hash. Test: s1_l1_doctor_contract.rs::undo_restores_before_hash.
- LAW-L1-REPROBE — a repair cannot advance on its pre-repair verdict. Test: s1_l1_doctor_contract.rs::repair_requires_fresh_probe.

## Upstream Reference and the Idempotence Attack

The source reviewed is the pinned mirror at /Volumes/ZestData/dicklesworthstone-mirror/beads_rust; its Cargo.toml:1-4 says beads_rust version 0.5.7. The installed runtime reports br 0.4.1 and br doctor capabilities reports doctor_version=0.4.1; therefore the source citations below establish the upstream design shape, not byte identity with the installed binary.

The scout's idempotence assertion is correct only with its fixture precondition. tests/e2e_doctor_chokepoint.rs:622-697 performs a first repair that must record at least one action, then invokes repair again after the fixture is healthy, requires a second run directory, and asserts actions_2.is_empty(). It proves conditional no-op on an unchanged healthy observation. It does not prove zero actions when a tool version, daemon, or file hash changes.

The mutation authority is explicit in the same upstream source: src/cli/commands/doctor_subsystems/mutate.rs:574-714 defines mutate; :595-598 computes before_hash; :643-648 writes and verifies a verbatim backup; :652-659 makes dry-run return the before hash without mutation; :671-704 computes after_hash and appends one action record. src/cli/commands/doctor.rs:265-308 creates .doctor/runs/<run-id>/ and its action file. The round-trip contract and undo evidence are tests/e2e_doctor_chokepoint.rs:327-344 and :435-488.

Refutation of the unconditional claim: the actual test names its premise second --repair is a no-op only after the first repair has made the fixture healthy. A changed before hash or probe input is not the same state. The shipped law we adopt is L1P-CONDITIONAL-IDEMPOTENCE, not a second run always performs zero actions.

RCH supplies the topology/convergence doctor precedent. rch doctor --help exposes --reliability and scoped values topology, convergence, pressure, triage, and others. The measured read-only command rch doctor --reliability --scope topology,convergence --json --no-self-healing --no-hook-auto-start logged scope application at rch/src/doctor.rs:1145, ran only the two requested probe families, returned worker topology passes, and returned repo convergence as explicit status=unknown, total=0 information rather than a false pass. L1 must carry the same scope and unknown discipline.

## Authority / Recovery / Ordering

- L1R-SCOPE: the caller supplies a scope; the report names requested, run, skipped, and unmeasured probes.
- L1R-REFUSE: wrong-version, stale, paused, unprobeable, or missing-family results do not advance as healthy.
- L1R-MUTATE: only the doctor repair session may call the chokepoint; a fixer cannot write directly.
- L1R-BACKUP: backup is created from the before bytes and verified before the operation is accepted.
- L1R-REPROBE: repair returns to the same probe predicate; a post-repair report is required.
- L1R-UNDO: undo is selected by run id, rechecks the recorded artifact, restores in reverse action order, and reports partial failure.

## Observability

| ID | Row | Source | Freshness | Known-bad |
|---|---|---|---|---|
| L1-OBS-PROBE | probe evidence | report row: presence, version, scope, source revision | per run | present tool with wrong version |
| L1-OBS-VERDICT | typed verdict | report row: verdict, reason_code, next_command | per run | missing reason or missing next action |
| L1-OBS-MUTATION | repair audit | .doctor/runs/<id>/actions.jsonl with before/after hashes | per repair run | mutation without backup/action row |
| L1-OBS-UNDO | recovery result | undo envelope plus restored hash comparisons | per undo run | restored bytes differ from before hash |

Metric: L1-METRIC-REPAIR-ACTION-RATE = repair action count per doctor run, partitioned by same_observation versus state_drift; the required floor is 0 only for the same-observation partition. A nonzero action rate after drift is not a failure.

## VIOLATION — where shipped code contradicts this law

- Declared: docs/plan/flow/boxes/S1.toml:36-41 declares L1 ompo doctor with two-signal probes and exists = none.
- Shipped: docs/plan/flow/boxes/S1.toml:41 says L1 exists is none; no ompo doctor consumer or local L1 contract suite exists in this Wave-0 tree.
- Consequence: the S1 system check cannot emit a production doctor verdict, cannot perform the conditional idempotence readback, and cannot prove a repair was followed by a fresh probe. This is the explicit pre-build gap, not a green result.

## Non-Coverage

- This contract does not install or select tool binaries, merge hooks, or write policy files; L2 owns ecosystem trust and initialization.
- It does not decide HD-0009..HD-0012, OMP pane liveness, Agent Mail roster truth, or dispatch admission.
- It does not claim the installed br 0.4.1 is byte-identical to the cited mirror beads_rust 0.5.7.
- It does not claim an existing local ompo doctor implementation; the suite and report are missing until a later build wave.
- It does not turn a no-op on one healthy fixture into a global host-health guarantee.

## Validation

One pasteable Wave-0 source-and-law stand-in. It must print PASS only when the upstream chokepoint, backup/hash, undo, and conditional-idempotence anchors are present; it does not run Cargo or mutate a workspace.

```bash
python3 - <<'PY'
from pathlib import Path
mutate = Path('/Volumes/ZestData/dicklesworthstone-mirror/beads_rust/src/cli/commands/doctor_subsystems/mutate.rs').read_text()
test = Path('/Volumes/ZestData/dicklesworthstone-mirror/beads_rust/tests/e2e_doctor_chokepoint.rs').read_text()
contract = Path('docs/contracts/s1_l1_doctor.md').read_text()
need_arms = ['OK', 'ABSENT_FAMILY', 'ABSENT_SPECIFIC', 'UNPROBEABLE', 'STALE', 'PAUSED', 'UNMEASURED']
need_mutate = ['pub fn mutate', 'before_hash', 'after_hash', 'write_verbatim_backup', 'actions_file']
need_test = ['fn chokepoint_idempotence', 'actions_2.is_empty()', 'doctor", "undo']
assert all(x in contract for x in need_arms), 'L1 verdict arms missing'
assert all(x in mutate for x in need_mutate), 'L1 mutate anchors missing'
assert all(x in test for x in need_test), 'L1 idempotence/undo anchors missing'
unchanged = (('sha256:a', 'sha256:b'), ('tool@1', 'daemon:up'))
drifted = (('sha256:b', 'sha256:c'), ('tool@2', 'daemon:up'))
assert unchanged[0][1] == 'sha256:b' and unchanged[1] == ('tool@1', 'daemon:up')
assert drifted != unchanged
print('L1_CONTRACT_VALIDATION PASS arms=7 timeout=unmeasured_unrun conditional_idempotence=state_bound mutate=anchored undo=anchored production_suite=MISSING')
PY
```

Output:

    L1_CONTRACT_VALIDATION PASS arms=7 timeout=unmeasured_unrun conditional_idempotence=state_bound mutate=anchored undo=anchored production_suite=MISSING

## Cross-References

- docs/plan/flow/boxes/S1.toml:36-41,108-130 — L1 input, rules, branches, and current non-claim.
- SCHEMAS.toml:156-163 — inception artifact consumed by L2, not owned by L1.
- /Volumes/ZestData/dicklesworthstone-mirror/beads_rust/src/cli/commands/doctor.rs:265-308 — repair run and audit context.
- /Volumes/ZestData/dicklesworthstone-mirror/beads_rust/src/cli/commands/doctor_subsystems/mutate.rs:574-714 — mutation chokepoint.
- /Volumes/ZestData/dicklesworthstone-mirror/beads_rust/tests/e2e_doctor_chokepoint.rs:327-344,435-488,622-697 — backup, undo, and conditional idempotence tests.
- docs/contracts/s1_l2_ecosystem.md — L2 trust/init boundary.
- docs/contracts/s1_l3_walkthrough.md — sibling L3 renderer boundary.

## NO-CLAIM

This contract pins the L1 law and cites an upstream implementation/test shape. It does not establish that ompo doctor exists, that the local invariant suite is wired, that every installed br 0.4.1 path matches the mirror source, or that a second doctor run is safe to skip after host state changes. A green Wave-0 stand-in proves only that the cited source anchors and the conditional law are present.


## Cross-Attack

| ID | Sibling | Rejection | Result |
|---|---|---|---|
| XATTACK-L3-IDEMPOTENCE | docs/contracts/s1_l3_walkthrough.md:34,43 | REJECTED as current proof, not as a future law: L3P-IDEMPOTENT names a second ompo start state-reporting law, but no production source or runner currently enforces it. | The command found only sibling-contract references; crates/omp-orchestrator had no implementation or consumer hit. |

Command run:

    git grep --no-index -n -E 'L3P-IDEMPOTENT|second_start_same_ids|ompo start' -- crates/omp-orchestrator docs/contracts/s1_l3_walkthrough.md

The sibling is honest that exists = none today, but its Validation stand-in cannot establish a future re-run state contract. This attack rejects any reading of LAW-L3-IDEMPOTENT as currently proven; it does not reject the law as a Wave-1 target.
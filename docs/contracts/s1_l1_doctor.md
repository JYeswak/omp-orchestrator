# S1 L1 Doctor Contract

Bead: omp-orchestrator-s1w0-l1l2-doctor-init-contract-d9rv

## Purpose

L1 defines the scoped system doctor for the S1 human-start path: it probes required tools with independent presence and version evidence, distinguishes absent, unprobeable, stale, paused, and healthy results, exposes a typed report and next action, and performs only explicit, reversible repairs through one mutation chokepoint. Its idempotence law is conditional: an unchanged observation with identical before-state hashes produces zero repair actions on the next run; a changed tool, daemon, or hash is a new observation and may require repair.

## Contract Artifacts

1. Canonical artifact: the current observer emits ompo.doctor.v1 JSON on stdout and durably writes
   omp.lifecycle_event.v1 rows to the repo journal. The report.json path remains a declared
   next-artifact reference; this bead does not claim that file is persisted.
2. Smoke runner: implemented as ompo doctor --json --scope system; its JSON carries a typed
   two-band exit_code and every probe row carries independent presence and version fields.
3. Invariant suite: crates/ompo-doctor/tests/l1_doctor.rs plus the production-path unit tests in
   crates/ompo-doctor/src/lib.rs. The suite exercises healthy, missing-version, unprobeable,
   absent, mutation, and empty-set outcomes.

The remaining repair, seven-arm verdict, and idempotence rows below are not claimed by this
observability slice.

## L1 Doctor Model

| ID | Property | Description |
|---|---|---|
| `L1-INPUT` | evidence | One probe has name, family, presence observation, version observation, scope, and provenance. |
| `L1-VERDICT` | typed result | Verdict is OK, ABSENT_FAMILY, ABSENT_SPECIFIC, UNPROBEABLE, STALE, PAUSED, or UNMEASURED; no missing value becomes OK. |
| `L1-REPAIR` | authority | Every repair enters one mutate() chokepoint, with a run directory, backup, before hash, after hash, and action record. |
| `L1-UNDO` | recovery | undo <run-id> restores the recorded pre-mutation bytes or database snapshot, and reports the result. |
| `L1-IDEMPOTENCE` | conditional law | Same scope plus identical before-state hashes and unchanged probe inputs yields zero repair mutations on the next doctor run. |
| `L1-DRIFT` | re-evaluation | A changed version, daemon state, scope, or before hash invalidates the no-op premise; the doctor must re-probe and may repair. |
| `L1-SCOPE` | bounded work | A scoped run executes only named probe families; skipped probes are explicit, never silently green. |

## Probe Verdict Type

ProbeVerdict is one seven-arm semantic enum (PROJECTED from vv9h). The UNMEASURED arm carries a run_state and reason_code so timeout and instrument failures remain Unrun-like without becoming a false refusal.

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

- `L1P-EXHAUSTIVE-ARMS`: every probe result is exactly one of the seven arms above; absence of a matching arm is a decode/instrument error.
- `L1P-WRONG-VERSION-STALE`: presence plus a wrong version is STALE, never OK, ABSENT_SPECIFIC, or UNPROBEABLE.
- `L1P-UNRUN-NOT-REFUSED`: timeout, no-record, and instrument-error observations are UNMEASURED and never Refused.
- `L1P-PAUSED-DISTINCT`: an explicit operator pause is PAUSED, not UNPROBEABLE, so resume intent is preserved.
- `L1P-HD-NAMED`: only an explicit named HD may convert UNPROBEABLE, STALE, PAUSED, or UNMEASURED into degraded continuation; the enum arm and reason remain visible.

### Verdict laws

- `LAW-L1-VERDICT-EXHAUSTIVE` — the decoder cannot silently map an unknown result to OK or Refused. Test: `l1_doctor.rs::unknown_arm_is_instrument_error`.
- `LAW-L1-UNRUN` — a timeout produces UNMEASURED with run_state=UNRUN and never Pass or Refused. Test: `l1_doctor.rs::timeout_is_unrun`.
- `LAW-L1-WRONG-VERSION` — a present tool below the version floor produces STALE. Test: `l1_doctor.rs::wrong_version_is_stale`.
- `LAW-L1-PAUSED` — an explicit pause produces PAUSED and retains pause intent. Test: `l1_doctor.rs::paused_is_not_unprobeable`.
- `LAW-L1-UNKNOWN` — no authoritative record produces UNMEASURED with reason_code=UNKNOWN_NO_RECORD, not UNPROBEABLE. Test: `l1_doctor.rs::unknown_record_is_unmeasured`.

### Properties

- `L1P-TWO-SIGNALS`: a tool is OK only when presence and version evidence both satisfy the required contract.
- `L1P-UNKNOWN-LOUD`: UNPROBEABLE, STALE, PAUSED, and UNMEASURED remain typed outcomes and carry a reason plus next action.
- `L1P-ONE-MUTATOR`: no fixer writes outside the mutation chokepoint.
- `L1P-BEFORE-HASH`: every reversible file mutation records the exact pre-state hash before writing a backup or applying the operation.
- `L1P-CONDITIONAL-IDEMPOTENCE`: zero actions is required only when the second run observes the same scope, probe inputs, and before-state hash tuple as the first run's post-state. It is not a promise that a doctor never acts twice.
- `L1P-REPAIR-THEN-REPROBE`: a repair is not a healthy verdict; the same probe must be run again before L1 advances.

## Laws

- `LAW-L1-SCOPED-PROBE` — a scoped doctor emits only requested probe families and preserves explicit skipped/unmeasured outcomes. Test: `l1_doctor.rs::scope_does_not_vacuously_pass_skipped_probes`.
- `LAW-L1-TWO-SIGNALS` — a present tool at the wrong version is not OK. Test: `l1_doctor.rs::wrong_version_is_stale`.
- `LAW-L1-MUTATE-AUDIT` — a repair records run id, before hash, backup, after hash, and one action row. Test: `l1_doctor.rs::repair_records_before_hash_backup_after_hash`.
- `LAW-L1-CONDITIONAL-IDEMPOTENCE` — identical post-repair state and probe inputs produce zero second-run actions; changed state reopens repair. Test: `l1_doctor.rs::second_repair_is_zero_actions_given_identical_hashes`.
- `LAW-L1-UNDO` — undo restores every touched target to its recorded before hash. Test: `l1_doctor.rs::undo_restores_before_hash`.
- `LAW-L1-REPROBE` — a repair cannot advance on its pre-repair verdict. Test: `l1_doctor.rs::failed_reprobe_blocks_l2`.

## Build Inventory

Every row below is one L1 build item. The WORKTREE inventory count is 20 BUILD rows and 22 TEST rows; the eleven-probe count is WORKTREE S1.toml:39-40; fh is intentionally excluded after Joshua removed it at f735cfd. The listed command is the acceptance shape for the future build; no L1 crate is built in this readiness pass.

| ID | Build item | Run -> expect |
|---|---|---|
| `L1-BUILD-DOCTOR` | DoctorReport runner and JSON envelope | Run ompo doctor --json; expect schema, run id, scope, probes, verdicts, remediation, and next action. |
| `L1-BUILD-SCOPE` | Explicit probe-family scope | Run ompo doctor --scope system; expect only requested probes and explicit skipped/unmeasured rows. |
| `L1-BUILD-PROBE-TMUX` | tmux probe | Run tmux --version; expect presence plus version evidence. |
| `L1-BUILD-PROBE-NTM` | ntm probe | Run ntm --version or supported health; expect version/presence or UNPROBEABLE, never guessed OK. |
| `L1-BUILD-PROBE-BR` | br probe | Run br --version; expect presence plus required version. |
| `L1-BUILD-PROBE-BV` | bv probe | Run bv --version or supported health; expect presence plus required version. |
| `L1-BUILD-PROBE-AGENT-MAIL` | Agent Mail probe | Run the supported health/version probe; expect endpoint identity and version or typed absence. |
| `L1-BUILD-PROBE-SOCRATICODE` | socraticode/qdrant probe | Run the supported health probe; expect service and index state, not a process-only assumption. |
| `L1-BUILD-PROBE-RCH` | rch plus repo-lane probe | Run rch doctor scoped to topology/convergence; expect lane identity and explicit UNKNOWN when no repo record exists. |
| `L1-BUILD-PROBE-GIT` | git probe | Run git rev-parse --show-toplevel; expect repository identity or ABSENT_FAMILY. |
| `L1-BUILD-PROBE-DISK` | disk versus mint-floor probe | Run the read-only disk/mint probe; expect measured capacity and remediation, never a guessed green. |
| `L1-BUILD-PROBE-FRANKENMERMAID` | frankenmermaid probe | Run frankenmermaid --version or health; expect presence and version. |
| `L1-BUILD-PROBE-TOOLCHAIN` | toolchain-pin probe | Read rust-toolchain.toml and supported toolchain identity; expect pin or remediation. |
| `L1-BUILD-TWO-SIGNALS` | presence AND version evaluator | Run a present wrong-version fixture; expect STALE, never OK or ABSENT. |
| `L1-BUILD-VERDICT` | ProbeVerdict seven-arm wire type (PROJECTED; vv9h) | Run the vv9h contract; expect OK, ABSENT_FAMILY, ABSENT_SPECIFIC, UNPROBEABLE, STALE, PAUSED, or UNMEASURED with run state/reason. |
| `L1-BUILD-REMEDIATION` | per-ABSENT remediation table | Run every absent-family/specific case; expect a non-empty remediation or requiredness-driven halt. |
| `L1-BUILD-EXIT` | two-band exit envelope | Run healthy and refusing probes; expect exit 0 for healthy and nonzero typed outcome without conflating UNRUN with refusal. |
| `L1-BUILD-MUTATE` | repair mutation chokepoint | Run a repair; expect before hash, verbatim backup, after hash, and one action record. |
| `L1-BUILD-UNDO` | undo/restore path | Run undo for a repair run; expect restored bytes to match the recorded before hash. |
| `L1-BUILD-REPROBE-IDEMPOTENCE` | post-repair re-probe and conditional idempotence | Run repair twice with unchanged hashes, then after a changed hash; expect zero second actions only in the unchanged case and a reopened repair after drift. |

## Test Inventory

Every row below is one L1 test item. Each name is the future single-pass test function and each known-bad leg is mandatory.

| ID | Named test | Run -> expect; known-bad |
|---|---|---|
| `L1-TEST-PROBE-TMUX` | l1_doctor.rs::tmux_probe_emits_two_signals | Run tmux fixture; expect presence/version row; wrong version is STALE. |
| `L1-TEST-PROBE-NTM` | l1_doctor.rs::ntm_probe_is_typed | Run ntm probe fixture; expect version or UNPROBEABLE; unsupported answer must not be OK. |
| `L1-TEST-PROBE-BR` | l1_doctor.rs::br_probe_emits_two_signals | Run br fixture; expect presence/version; missing binary is ABSENT with remediation. |
| `L1-TEST-PROBE-BV` | l1_doctor.rs::bv_probe_emits_two_signals | Run bv fixture; expect presence/version; wrong version is STALE. |
| `L1-TEST-PROBE-AGENT-MAIL` | l1_doctor.rs::agent_mail_probe_is_scoped | Run endpoint fixture with an unresponsive endpoint; expect UNMEASURED/UNKNOWN_NO_RECORD. KNOWN-BAD: present endpoint with no version response must not be OK. |
| `L1-TEST-PROBE-SOCRATICODE` | l1_doctor.rs::socraticode_probe_preserves_unknown | Run service fixture with no index record; expect UNMEASURED UNKNOWN_NO_RECORD. |
| `L1-TEST-PROBE-RCH` | l1_doctor.rs::rch_probe_reports_lane_state | Run RCH topology fixture; expect explicit repo UNKNOWN when no record exists. |
| `L1-TEST-PROBE-GIT` | l1_doctor.rs::git_probe_rejects_non_repo | Run outside a repo; expect ABSENT_FAMILY with remediation. |
| `L1-TEST-PROBE-DISK` | l1_doctor.rs::disk_probe_reports_floor | Run low-floor fixture; expect typed pressure/remediation, not green. |
| `L1-TEST-PROBE-FRANKENMERMAID` | l1_doctor.rs::frankenmermaid_probe_emits_two_signals | Run wrong-version fixture; expect STALE. |
| `L1-TEST-PROBE-TOOLCHAIN` | l1_doctor.rs::toolchain_probe_requires_pin | Run missing/invalid pin fixture; expect ABSENT_SPECIFIC plus remediation. |
| `L1-TEST-TWO-SIGNAL` | l1_doctor.rs::wrong_version_is_stale | Run present tool below floor; expect STALE, not OK or absent. |
| `L1-TEST-VERDICT-ARMS` | l1_doctor.rs::every_probe_verdict_has_branch | Run all seven-arm fixtures (PROJECTED from vv9h); expect each declared branch. KNOWN-BAD: inject an unknown arm; expect decode/instrument error, never default green. |
| `L1-TEST-TIMEOUT-UNRUN` | l1_doctor.rs::timeout_is_unmeasured_unrun | Run timeout fixture; expect UNMEASURED run_state=UNRUN, never Pass or Refused. |
| `L1-TEST-ABSENT-REMEDIATION` | l1_doctor.rs::every_absent_has_remediation | Run absent family and absent specific fixtures; expect remediation or requiredness halt. KNOWN-BAD: emit ABSENT with empty remediation; expect test failure. |
| `L1-TEST-EXIT-LATTICE` | l1_doctor.rs::exit_bands_match_verdicts | Run healthy/refused/unrun fixtures; expect distinct exit bands. KNOWN-BAD: map UNRUN to Refused; expect test failure. |
| `L1-TEST-MUTATE-BACKUP` | l1_doctor.rs::repair_records_before_hash_backup_after_hash | Run repair fixture; expect backup/action evidence; delete backup leg must fail. |
| `L1-TEST-UNDO` | l1_doctor.rs::undo_restores_before_hash | Run repair then undo; expect byte/hash restoration; tampered backup must refuse. |
| `L1-TEST-IDEMPOTENT-SAME` | l1_doctor.rs::second_repair_is_zero_actions_given_identical_hashes | Run repair twice without drift; expect second action count 0. KNOWN-BAD: change target before-hash between runs; expect a new action, not zero. |
| `L1-TEST-IDEMPOTENT-DRIFT` | l1_doctor.rs::second_repair_reopens_on_changed_before_hash | Change a target hash between runs; expect a new repair action, not a false no-op. |
| `L1-TEST-REPROBE-HALT` | l1_doctor.rs::failed_reprobe_blocks_l2 | Make repair readback fail; expect L2 not reached. |
| `L1-TEST-SCOPE-UNKNOWN` | l1_doctor.rs::scope_does_not_vacuously_pass_unknown | Omit a requested probe record; expect UNMEASURED/UNKNOWN, not Pass. |

## Dispatch Preflight (L1)

1. **How it flows:** FILE the build/test bead with run-X-expect-Y acceptance; CLAIM it to the L1 lane; PACKET its exact contract row; ADMISSION checks the current freeze and required predecessor; SEND only after admission; RECEIPT and ACK establish delivery; OBSERVE the worker result; VERIFY with the named non-author test; RECORD the lifecycle event and evidence.
2. **What must be true:** the contract row, named test, upstream source identity, RCH lane, current repo revision, and known-bad fixture must all be present; no build may start while its acceptance or source pin is missing.
3. **If automated:** bv selects the ready row, bead-availability confirms claimability, dispatch-claim-fence confirms assignment, and the L1 gate reads the typed report before allowing the next edge. A missing or UNKNOWN oracle is a refusal to advance, not an empty queue.
4. **Escape route:** an agent reports OK from presence alone when the version probe is UNPROBEABLE, collapsing UNPROBEABLE/STALE into green. The two-signal test and L1P-UNRUN-NOT-REFUSED are the detector.

## Upstream Reference and the Idempotence Attack

The source reviewed is the pinned mirror at /Volumes/ZestData/dicklesworthstone-mirror/beads_rust; its Cargo.toml:1-4 says beads_rust version 0.5.7. The installed runtime reports br 0.4.1 and br doctor capabilities reports doctor_version=0.4.1; therefore the source citations below establish the upstream design shape, not byte identity with the installed binary.

The scout's idempotence assertion is correct only with its fixture precondition. tests/e2e_doctor_chokepoint.rs:622-697 performs a first repair that must record at least one action, then invokes repair again after the fixture is healthy, requires a second run directory, and asserts actions_2.is_empty(). It proves conditional no-op on an unchanged healthy observation. It does not prove zero actions when a tool version, daemon, or file hash changes.

The mutation authority is explicit in the same upstream source: src/cli/commands/doctor_subsystems/mutate.rs:574-714 defines mutate; :595-598 computes before_hash; :643-648 writes and verifies a verbatim backup; :652-659 makes dry-run return the before hash without mutation; :671-704 computes after_hash and appends one action record. src/cli/commands/doctor.rs:265-308 creates .doctor/runs/<run-id>/ and its action file. The round-trip contract and undo evidence are tests/e2e_doctor_chokepoint.rs:327-344 and :435-488.

Refutation: e2e_doctor_chokepoint.rs:622-697 proves zero second actions only after the first run leaves a healthy fixture; changed hash or probe input reopens repair. Adopt `L1P-CONDITIONAL-IDEMPOTENCE`, not global zero actions.

RCH's doctor exposes scoped reliability probes: --scope topology,convergence runs only those families and reports repo convergence status=unknown when records are absent. That UNKNOWN discipline is required here.

## Authority / Recovery / Ordering

- `L1R-SCOPE`: the caller supplies a scope; the report names requested, run, skipped, and unmeasured probes.
- `L1R-REFUSE`: wrong-version, stale, paused, unprobeable, or missing-family results do not advance as healthy.
- `L1R-MUTATE`: only the doctor repair session may call the chokepoint; a fixer cannot write directly.
- `L1R-BACKUP`: backup is created from the before bytes and verified before the operation is accepted.
- `L1R-REPROBE`: repair returns to the same probe predicate; a post-repair report is required.
- `L1R-UNDO`: undo is selected by run id, rechecks the recorded artifact, restores in reverse action order, and reports partial failure.

## Observability

| ID | Row | Source | Freshness | Known-bad |
|---|---|---|---|---|
| `OBS-L1-PROBE` | probe evidence | report row: presence, version, scope, source revision | per run | present tool with wrong version |
| `OBS-L1-VERDICT` | typed verdict | report row: verdict, reason_code, next_command | per run | missing reason or missing next action |
| `OBS-L1-MUTATION` | repair audit | .doctor/runs/<id>/actions.jsonl with before/after hashes | per repair run | mutation without backup/action row |
| `OBS-L1-UNDO` | recovery result | undo envelope plus restored hash comparisons | per undo run | restored bytes differ from before hash |

Metric: `MET-L1-REPAIR-ACTION-RATE` = repair action count per doctor run, partitioned by same_observation versus state_drift; the required floor is 0 only for the same-observation partition. A nonzero action rate after drift is not a failure.

## Superseded Wave-0 violation and current slice

- Historical: the Wave-0 contract and S1 flow map recorded the doctor as missing. The current
  ompo-doctor crate now owns the declared L1 probe writer and its umbrella command.
- Current slice: ompo doctor emits one lifecycle row per declared probe, records independent PATH
  presence and successful version evidence, and returns the two subject-result bands (0 or 1).
- Remaining: the full seven-arm verdict enum, repair chokepoint, undo, and conditional idempotence
  remain outside this observability bead and are not promoted by its tests.

## Non-Coverage

- This contract does not install or select tool binaries, merge hooks, or write policy files; L2 owns ecosystem trust and initialization.
- It does not decide HD-0009..HD-0012, OMP pane liveness, Agent Mail roster truth, or dispatch admission.
- It does not claim the installed br 0.4.1 is byte-identical to the cited mirror beads_rust 0.5.7.
- This slice does claim the existing ompo doctor observer and its L1 tests; it does not claim the
  report.json artifact, seven-arm verdict enum, repair, undo, or idempotence rows above.
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
def repair_actions(previous_after_hash, observed_before_hash):
    return 0 if previous_after_hash == observed_before_hash else 1
assert repair_actions('sha256:b', 'sha256:b') == 0
assert repair_actions('sha256:b', 'sha256:c') == 1
print('L1_CONTRACT_VALIDATION PASS arms=7 timeout=unmeasured_unrun conditional_idempotence=state_bound drift_leg=injected mutate=anchored undo=anchored production_suite=MISSING')
PY
```

Output:

    L1_CONTRACT_VALIDATION PASS arms=7 timeout=unmeasured_unrun conditional_idempotence=state_bound drift_leg=injected mutate=anchored undo=anchored production_suite=MISSING

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
| `XATTACK-L3-IDEMPOTENCE` | docs/contracts/s1_l3_walkthrough.md:34,43 | REJECTED as current proof, not as a future law: L3P-IDEMPOTENT names a second ompo start state-reporting law, but no production source or runner currently enforces it. | The command found only sibling-contract references; crates/omp-orchestrator had no implementation or consumer hit. |

Command run:

    git grep --no-index -n -E 'L3P-IDEMPOTENT|second_start_same_ids|ompo start' -- crates/omp-orchestrator docs/contracts/s1_l3_walkthrough.md

The sibling is honest that exists = none today, but its Validation stand-in cannot establish a future re-run state contract. This attack rejects any reading of LAW-L3-IDEMPOTENT as currently proven; it does not reject the law as a Wave-1 target.
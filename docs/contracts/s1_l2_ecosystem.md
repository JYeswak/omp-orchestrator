# S1 L2 Ecosystem Contract

Bead: omp-orchestrator-s1w0-l1l2-doctor-init-contract-d9rv

## Purpose

L2 defines the trusted ecosystem check and initialization boundary for S1: it identifies the repository and host, checks required control files and project state, refuses to overwrite foreign policy without explicit opt-in, snapshots any target before mutation, writes an inception manifest and S1 FOUNDATION row, then re-probes the resulting state before L3. Initialization is idempotent only for an unchanged observed state; a new tool version, missing daemon, changed policy file, or changed hash requires a fresh decision and may require repair.

## Contract Artifacts

1. Canonical artifact: .omp-orchestrator/inception.json plus the S1 row in docs/plan/FOUNDATION.jsonl; schema authority is SCHEMAS.toml:156-163.
2. Smoke runner: MISSING today; target ompo doctor --repo PATH --json followed by ompo init --repo PATH --json.
3. Invariant suite: MISSING today; target crates/installer/tests/s1_l2_ecosystem_contract.rs. The Wave-0 validation below is an executable contract stand-in, not production coverage.

A named but missing suite is intentional Wave-0 state, not a passing implementation claim.

## L2 Ecosystem Model

| ID | Property | Description |
|---|---|---|
| L2-IDENTITY | authority | Repository identity includes canonical root, repository marker, and source revision; a path alone is insufficient. |
| L2-TRUST | refusal | Existing AGENTS.md, CLAUDE.md, hooks, and registries are foreign policy until an explicit init decision authorizes changes. |
| L2-SNAPSHOT | recovery | Every target write has a before hash, verbatim backup or equivalent durable snapshot, and an action row. |
| L2-INCEPTION | output | inception.json contains schema_version, project_id, repo_identity, control_files, host_capabilities, required_tools, and trust_status. |
| L2-FOUNDATION | lineage | The S1 FOUNDATION.jsonl row names the inception artifact in output_refs and records known, unknown, and gaps without blank epistemic cells. |
| L2-REPROBE | postcondition | Init re-runs the relevant doctor predicates; failed readback halts and never advances to L3. |
| L2-SCOPE | bounded work | Ecosystem probes run by explicit scope and retain UNKNOWN/UNPROBEABLE results; skipped checks are not green. |
| L2-IDEMPOTENCE | conditional law | Same repository identity, control-file hashes, template hash, and probe inputs produce zero second-run writes; state drift reopens the decision. |

### Properties

- L2P-IDENTITY-BEFORE-WRITE: identity and trust are resolved before any control-file write.
- L2P-NO-FOREIGN-OVERWRITE: missing or unstamped policy files require explicit trusted-init; absence of opt-in is a human halt.
- L2P-BACKUP-BEFORE-HASH: the target's before hash is recorded and backed up before mutation.
- L2P-INCEPTION-COMPLETE: all required inception fields are present, and each value is classified known, unknown, or gap.
- L2P-FOUNDATION-LINK: the S1 FOUNDATION row and inception manifest refer to each other through stable output/input references.
- L2P-REPROBE: init success is a fresh post-write observation, not the write call's return value.
- L2P-CONDITIONAL-IDEMPOTENCE: no second write is required only when the complete observed pre-state matches; a tool upgrade or policy drift is a legitimate new repair case.
- L2P-SCOPE-UNKNOWN: an ecosystem doctor scoped to topology/convergence does not claim host policy or hooks were checked.

## Laws

- LAW-L2-IDENTITY — foreign or ambiguous repository identity refuses before mutation. Test: s1_l2_ecosystem_contract.rs::ambiguous_identity_halts.
- LAW-L2-TRUSTED-INIT — an unstamped foreign AGENTS.md/CLAUDE.md without opt-in emits a human halt and writes nothing. Test: s1_l2_ecosystem_contract.rs::foreign_policy_without_opt_in_is_read_only.
- LAW-L2-BACKUP — trusted init records before hash and durable backup before writing a target. Test: s1_l2_ecosystem_contract.rs::init_backup_precedes_write.
- LAW-L2-INCEPTION — accepted init writes every required inception field and the S1 FOUNDATION row. Test: s1_l2_ecosystem_contract.rs::inception_and_foundation_are_linked.
- LAW-L2-REPROBE — init must re-run doctor predicates and halt when the stamped state does not read back. Test: s1_l2_ecosystem_contract.rs::failed_reprobe_halts.
- LAW-L2-CONDITIONAL-IDEMPOTENCE — same complete observed state produces zero second-run writes, while changed policy/tool/daemon state is re-evaluated. Test: s1_l2_ecosystem_contract.rs::same_hash_noop_but_drift_reopens.
- LAW-L2-SCOPE — scoped topology/convergence checks do not imply policy, hook, or repository identity checks. Test: s1_l2_ecosystem_contract.rs::scope_preserves_unknowns.

## Upstream Reference and Boundary

The upstream br doctor source is the mirror at /Volumes/ZestData/dicklesworthstone-mirror/beads_rust (manifest version 0.5.7); the installed CLI is br 0.4.1, so this is a cited source shape, not an installed-version identity claim. Its repair session creates .doctor/runs/<run-id>/ at src/cli/commands/doctor.rs:265-308. Its single chokepoint at src/cli/commands/doctor_subsystems/mutate.rs:574-714 computes before_hash at :595-598, verifies a verbatim backup at :643-648, and records after_hash plus an action at :671-704. L2 reuses this recovery shape but adds repository identity, foreign-policy trust, inception schema, FOUNDATION lineage, and post-write readback.

The source test tests/e2e_doctor_chokepoint.rs:622-697 refutes an unconditional no-op interpretation: its second repair is zero-action only after the first repair made the fixture healthy. Any changed control-file hash, template hash, or probe result is a new observed state. L2 therefore adopts L2P-CONDITIONAL-IDEMPOTENCE, not init never writes on its second invocation.

RCH's live doctor exposes the boundary L2 needs: rch doctor --reliability --scope topology,convergence --json --no-self-healing --no-hook-auto-start applies only the requested scopes at rch/src/doctor.rs:1145; its result reports worker topology as pass and repository convergence as explicit unknown when no records exist. L2 must preserve that distinction: a healthy worker topology is not proof that this repository's policy or hooks are trusted.

## Authority / Recovery / Ordering

- L2R-IDENTITY: establish repo root, source revision, project id, and host capability provenance first.
- L2R-TRUST: existing policy is read-only until trusted_init=true is an explicit decision tied to the repository identity.
- L2R-BACKUP: take the target before hash and backup before any write; backup failure refuses the operation.
- L2R-INIT: write inception and FOUNDATION artifacts atomically or leave the prior state intact.
- L2R-REPROBE: re-run the same control-file and tool probes after writes; mismatch halts.
- L2R-UNDO: restore the run's before snapshot by run id, verify hashes, and expose partial recovery as a refusal.
- L2R-IDEMPOTENCE: compare complete before-state hashes and probe inputs, not merely invocation count.

## Observability

| ID | Row | Source | Freshness | Known-bad |
|---|---|---|---|---|
| L2-OBS-IDENTITY | repo identity | inception.repo_identity plus source revision | per run | path reused for a different repository |
| L2-OBS-TRUST | trust decision | trusted_init, policy hashes, decision id | per init attempt | foreign policy changed without opt-in |
| L2-OBS-WRITE | backup/write audit | run id, target, before/after hash, backup path | per mutation | target changed with no before hash |
| L2-OBS-READBACK | post-init state | inception.json, FOUNDATION S1 row, re-probe report | after every init | write returned success but re-probe fails |

Metric: L2-METRIC-INIT-READBACK-FAILURE-RATE = failed post-init readbacks divided by init attempts, partitioned by trusted_init and state-drift reason. The required floor is 0 for the same-state partition; drift-triggered refusal is not a false success.

## VIOLATION — where shipped code contradicts this law

- Declared: docs/plan/flow/boxes/S1.toml:43-49 declares L2 ecosystem checks, trusted initialization writes, and the inception target.
- Shipped: docs/plan/flow/boxes/S1.toml:49 says the crate is none; :48 lists intended writes but no local writer or readback consumer exists in this Wave-0 tree.
- Consequence: a foreign repository cannot currently receive a production identity/trust decision, backed initialization, inception manifest, FOUNDATION row, or post-write readback through ompo. Treating the planned writes as completed would be a false green.

## Non-Coverage

- L2 does not decide which tools are required; L1 supplies scoped probe verdicts.
- L2 does not render the L3 human/JSON walkthrough, determine OMP pane liveness, or dispatch agents.
- L2 does not install hooks or binaries in this wave; installation and hook merge remain explicit later work.
- It does not claim the current br 0.4.1 binary is byte-identical to the cited mirror beads_rust 0.5.7 source.
- It does not claim .omp-orchestrator/inception.json exists today; the current plan names it as a target and the local writer is missing.

## Validation

One pasteable Wave-0 source-and-law stand-in. It must print PASS only when the required S1 schema anchors and upstream recovery/idempotence anchors are present; it does not run Cargo or write repository state.

```bash
python3 - <<'PY'
from pathlib import Path
s1 = Path('docs/plan/flow/boxes/S1.toml').read_text()
mutate = Path('/Volumes/ZestData/dicklesworthstone-mirror/beads_rust/src/cli/commands/doctor_subsystems/mutate.rs').read_text()
test = Path('/Volumes/ZestData/dicklesworthstone-mirror/beads_rust/tests/e2e_doctor_chokepoint.rs').read_text()
need_s1 = ['trusted-init', 'inception.json', 'FOUNDATION.jsonl', 'jq readback']
need_mutate = ['before_hash', 'after_hash', 'write_verbatim_backup', 'actions_file']
need_test = ['fn chokepoint_idempotence', 'actions_2.is_empty()', 'doctor", "undo']
assert all(x in s1 for x in need_s1), 'L2 S1 anchors missing'
assert all(x in mutate for x in need_mutate), 'L2 mutation anchors missing'
assert all(x in test for x in need_test), 'L2 recovery/idempotence anchors missing'
first = {'repo': 'sha:a', 'policy': 'sha:p', 'template': 'sha:t', 'probe': 'tool@1'}
second_same = dict(first)
second_drift = {**first, 'probe': 'tool@2'}
assert second_same == first and second_drift != first
print('L2_CONTRACT_VALIDATION PASS trusted_init=anchored inception_foundation=anchored reprobe=anchored conditional_idempotence=state_bound production_suite=MISSING')
PY
```

Output:

    L2_CONTRACT_VALIDATION PASS trusted_init=anchored inception_foundation=anchored reprobe=anchored conditional_idempotence=state_bound production_suite=MISSING

## Cross-References

- docs/plan/flow/boxes/S1.toml:43-49,108-130 — L2 checks, writes, branches, and no-claim.
- SCHEMAS.toml:111-118,156-163 — FOUNDATION and inception artifact schemas.
- docs/plan/FOUNDATION.jsonl — S1 stage row and downstream S2 reference.
- /Volumes/ZestData/dicklesworthstone-mirror/beads_rust/src/cli/commands/doctor.rs:265-308 — upstream repair-run context.
- /Volumes/ZestData/dicklesworthstone-mirror/beads_rust/src/cli/commands/doctor_subsystems/mutate.rs:574-714 — upstream mutation authority.
- /Volumes/ZestData/dicklesworthstone-mirror/beads_rust/tests/e2e_doctor_chokepoint.rs:327-344,435-488,622-697 — backup, undo, and conditional idempotence.
- docs/contracts/s1_l1_doctor.md — L1 probe and repair boundary.
- docs/contracts/s1_l3_walkthrough.md — sibling L3 decision/walkthrough boundary.

## NO-CLAIM

These Wave-0 contracts pin L2's trust, backup, artifact, readback, and conditional-idempotence laws. They do not establish that ompo init, .omp-orchestrator/inception.json, or an invariant suite exists, that a current installed br 0.4.1 matches the mirror source, or that a second init may be skipped after any host or policy drift. Adoption, implementation, and S2 remain separate gates.

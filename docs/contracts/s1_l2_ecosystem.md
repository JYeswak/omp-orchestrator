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
| `L2-IDENTITY` | authority | Repository identity includes canonical root, repository marker, and source revision; a path alone is insufficient. |
| `L2-TRUST` | refusal | Existing AGENTS.md, CLAUDE.md, hooks, and registries are foreign policy until an explicit init decision authorizes changes. |
| `L2-SNAPSHOT` | recovery | Every target write has a before hash, verbatim backup or equivalent durable snapshot, and an action row. |
| `L2-INCEPTION` | output | inception.json contains schema_version, project_id, repo_identity, control_files, host_capabilities, required_tools, and trust_status. |
| `L2-FOUNDATION` | lineage | The S1 FOUNDATION.jsonl row names the inception artifact in output_refs and records known, unknown, and gaps without blank epistemic cells. |
| `L2-REPROBE` | postcondition | Init re-runs the relevant doctor predicates; failed readback halts and never advances to L3. |
| `L2-SCOPE` | bounded work | Ecosystem probes run by explicit scope and retain UNKNOWN/UNPROBEABLE results; skipped checks are not green. |
| `L2-IDEMPOTENCE` | conditional law | Same repository identity, control-file hashes, template hash, and probe inputs produce zero second-run writes; state drift reopens the decision. |

### Properties

- `L2P-IDENTITY-BEFORE-WRITE`: identity and trust are resolved before any control-file write.
- `L2P-NO-FOREIGN-OVERWRITE`: missing or unstamped policy files require explicit trusted-init; absence of opt-in is a human halt.
- `L2P-BACKUP-BEFORE-HASH`: the target's before hash is recorded and backed up before mutation.
- `L2P-INCEPTION-COMPLETE`: all required inception fields are present, and each value is classified known, unknown, or gap.
- `L2P-FOUNDATION-LINK`: the S1 FOUNDATION row and inception manifest refer to each other through stable output/input references.
- `L2P-REPROBE`: init success is a fresh post-write observation, not the write call's return value.
- `L2P-CONDITIONAL-IDEMPOTENCE`: no second write is required only when the complete observed pre-state matches; a tool upgrade or policy drift is a legitimate new repair case.
- `L2P-SCOPE-UNKNOWN`: an ecosystem doctor scoped to topology/convergence does not claim host policy or hooks were checked.

## Laws

- `LAW-L2-IDENTITY` — foreign or ambiguous repository identity refuses before mutation. Test: `l2_ecosystem.rs::identity_includes_root_and_revision`.
- `LAW-L2-TRUSTED-INIT` — unstamped foreign policy without opt-in emits a human halt and writes nothing. Test: `l2_ecosystem.rs::foreign_policy_without_opt_in_is_read_only`.
- `LAW-L2-BACKUP` — trusted init records before hash and durable backup before writing a target. Test: `l2_ecosystem.rs::init_backup_precedes_write`.
- `LAW-L2-INCEPTION` — accepted init writes every required inception field and the S1 FOUNDATION row. Test: `l2_ecosystem.rs::inception_and_foundation_are_linked`.
- `LAW-L2-REPROBE` — init re-runs doctor predicates and halts when stamped state does not read back. Test: `l2_ecosystem.rs::failed_reprobe_halts_l3`.
- `LAW-L2-CONDITIONAL-IDEMPOTENCE` — same complete state produces zero second-run writes; drift reopens review. Test: `l2_ecosystem.rs::second_init_is_zero_writes_given_identical_hashes`.
- `LAW-L2-SCOPE` — scoped topology/convergence checks do not imply policy, hooks, or identity checks. Test: `l2_ecosystem.rs::scope_preserves_unknowns`.

## Build Inventory

Every row below is one L2 build item. The WORKTREE inventory count is 18 BUILD rows and 22 TEST rows; these are the complete implementation rows, not a launch authorization. No figure is a TREE claim.

| ID | Build item | Run -> expect |
|---|---|---|
| `L2-BUILD-IDENTITY` | repository identity envelope | Run identity resolution; expect canonical root, source revision, project id, and host identity. |
| `L2-BUILD-GIT-REPO` | git repository check | Run git rev-parse --show-toplevel; expect a real repo or a typed halt. |
| `L2-BUILD-REMOTE-PERSONA-A` | local-only remote rule | Run Persona A with no remote; expect explicit remote_optional=true and continuation. |
| `L2-BUILD-REMOTE-PERSONA-BC` | fleet remote rule | Run Persona B/C with no remote; expect a named remediation/halt before shared dispatch. |
| `L2-BUILD-AGENTS-STAMP` | AGENTS.md stamp check | Run the control-file probe; expect stamp identity, source revision, and status. |
| `L2-BUILD-CLAUDE-STAMP` | CLAUDE.md stamp check | Run the control-file probe; expect stamp identity, source revision, and status. |
| `L2-BUILD-BEADS` | .beads initialization check | Run the tracker probe; expect project identity and writable state or remediation. |
| `L2-BUILD-RUST-TOOLCHAIN` | rust-toolchain.toml check | Run the pin probe; expect declared toolchain identity or remediation. |
| `L2-BUILD-HOOK-HEAD` | hook identity check | Run hook source/HEAD comparison; expect identity match or repair-required result. |
| `L2-BUILD-CARGO-MEMBERS` | Cargo member check | Run cargo metadata without building; expect declared members and no load error. |
| `L2-BUILD-AGENT-MAIL` | Agent Mail registration check | Run project/agent readback; expect registered identity or explicit remediation. |
| `L2-BUILD-RCH-LANE` | repo-to-RCH lane check | Run scoped RCH topology/convergence doctor; expect lane mapping or explicit UNKNOWN. |
| `L2-BUILD-TRUSTED-INIT-OPTIN` | trusted-init decision gate | Run foreign-policy init without consent; expect no write and HUMAN_HALT. |
| `L2-BUILD-TEMPLATE-IDENTITY` | template identity capture | Run opted-in init; expect template path, source hash, and template revision before write. |
| `L2-BUILD-BACKUP` | before-hash backup | Run opted-in init; expect target before hashes and durable backups before writes. |
| `L2-BUILD-INCEPTION-FOUNDATION` | inception and FOUNDATION writer | Run accepted init; expect inception.json and FOUNDATION.jsonl stage=S1 linked by refs. |
| `L2-BUILD-REPROBE` | post-init re-probe | Run accepted init; expect the same identity/control/tool predicates re-evaluated. |
| `L2-BUILD-HALT-NOT-TAKEN` | failed re-probe halt | Run init with a mutation that does not take; expect halt and no L3 advancement. |

## Test Inventory

Every row below is one L2 test item. Each has a named test and a branch-specific known-bad leg.

| ID | Named test | Run -> expect; known-bad |
|---|---|---|
| `L2-TEST-IDENTITY` | l2_ecosystem.rs::identity_includes_root_and_revision | Run foreign-root fixture; expect identity mismatch, not acceptance. |
| `L2-TEST-GIT-REPO` | l2_ecosystem.rs::non_repo_halts | Run outside git; expect typed halt with remediation. |
| `L2-TEST-REMOTE-A` | l2_ecosystem.rs::persona_a_allows_local_only_remote | Run Persona A without remote; expect allowed remote_optional=true continuation (known-good). Then run the same fixture as Persona B/C without remote; expect REMOTE_REQUIRED refusal and zero writes (known-bad). |
| `L2-TEST-REMOTE-BC` | l2_ecosystem.rs::persona_bc_requires_remote | Run Persona B/C without remote; expect halt/remediation. |
| `L2-TEST-AGENTS-STAMP` | l2_ecosystem.rs::agents_stamp_is_verified | Run wrong AGENTS stamp; expect mismatch, not stamped. |
| `L2-TEST-CLAUDE-STAMP` | l2_ecosystem.rs::claude_stamp_is_verified | Run wrong CLAUDE stamp; expect mismatch, not stamped. |
| `L2-TEST-BEADS` | l2_ecosystem.rs::beads_project_is_present | Run missing .beads; expect remediation/halt. |
| `L2-TEST-RUST-TOOLCHAIN` | l2_ecosystem.rs::toolchain_pin_is_present | Run missing/invalid rust-toolchain.toml; expect remediation. |
| `L2-TEST-HOOK-HEAD` | l2_ecosystem.rs::hook_identity_matches_head | Run stale hook; expect mismatch and repair-required. |
| `L2-TEST-CARGO-MEMBERS` | l2_ecosystem.rs::cargo_members_are_loadable | Run malformed member fixture; expect metadata failure, not empty success. |
| `L2-TEST-AGENT-MAIL` | l2_ecosystem.rs::agent_mail_registration_reads_back | Run unregistered project; expect explicit remediation. |
| `L2-TEST-RCH-LANE` | l2_ecosystem.rs::rch_lane_unknown_is_preserved | Run no repo convergence record; expect UNKNOWN, not absent or green. |
| `L2-TEST-TRUSTED-INIT` | l2_ecosystem.rs::foreign_policy_without_opt_in_is_read_only | Run unstamped foreign AGENTS.md without opt-in; expect HUMAN_HALT and zero writes. |
| `L2-TEST-TEMPLATE-IDENTITY` | l2_ecosystem.rs::trusted_init_records_template_identity | Run wrong template hash; expect refusal before write. |
| `L2-TEST-BACKUP` | l2_ecosystem.rs::init_backup_precedes_write | Run backup failure fixture; expect no target write. |
| `L2-TEST-INCEPTION-FOUNDATION` | l2_ecosystem.rs::inception_and_foundation_are_linked | Run missing field/ref fixture; expect validation failure. |
| `L2-TEST-REPROBE-SUCCESS` | l2_ecosystem.rs::successful_init_requires_reprobe | Run valid opt-in init; expect post-write probe evidence before success. KNOWN-BAD: return success without a fresh readback; expect test failure. |
| `L2-TEST-REPROBE-FAIL` | l2_ecosystem.rs::failed_reprobe_halts_l3 | Run init where stamp/readback remains wrong; expect halt, not L3. |
| `L2-TEST-IDEMPOTENT-SAME` | l2_ecosystem.rs::second_init_is_zero_writes_given_identical_hashes | Run init twice without drift; expect second write count 0. KNOWN-BAD: change policy hash between runs; expect repair/review, not no-op. |
| `L2-TEST-IDEMPOTENT-DRIFT` | l2_ecosystem.rs::second_init_reopens_on_policy_hash_drift | Change policy hash between runs; expect repair/review, not no-op. |
| `L2-TEST-EPISTEMIC-COMPLETE` | l2_ecosystem.rs::inception_has_no_blank_epistemic_cells | Run blank known/unknown/gap fixture; expect validation failure. |
| `L2-TEST-ATOMIC-ROLLBACK` | l2_ecosystem.rs::partial_init_rolls_back_atomically | Fail FOUNDATION append after inception write; expect prior state restored. |

## Dispatch Preflight (L2)

1. **How it flows:** FILE each L2 build/test bead with one run-X-expect-Y acceptance; CLAIM it to the L2 lane; PACKET its exact repository/trust scope; ADMISSION checks S1 approval and build prerequisites; SEND only after that verdict; RECEIPT and ACK establish delivery; OBSERVE the result; VERIFY with the named non-author test; RECORD the lifecycle evidence.
2. **What must be true:** L1 supplies typed probe outcomes; repository identity is stable; policy trust is explicit; the bead names a known-bad fixture; the required RCH lane and external source identities are available; no init writes occur without backup and a readback path.
3. **If automated:** bv selects the bead, bead-availability confirms it is claimable, a single mutation authority performs writes, and the post-init doctor re-probes inception/control files before the next stage. UNKNOWN or UNPROBEABLE is preserved and blocks healthy continuation.
4. **Escape route:** an agent treats a successful template write as proof of a trusted ecosystem, skips the before-hash backup or FOUNDATION readback, and advances on the write return code. The foreign-policy/no-opt-in and failed-reprobe tests must make that path fail closed.

## Upstream Reference and Boundary

The upstream br doctor source is the mirror at /Volumes/ZestData/dicklesworthstone-mirror/beads_rust (manifest version 0.5.7); the installed CLI is br 0.4.1, so this is a cited source shape, not an installed-version identity claim. Its repair session creates .doctor/runs/<run-id>/ at src/cli/commands/doctor.rs:265-308. Its single chokepoint at src/cli/commands/doctor_subsystems/mutate.rs:574-714 computes before_hash at :595-598, verifies a verbatim backup at :643-648, and records after_hash plus an action at :671-704. L2 reuses this recovery shape but adds repository identity, foreign-policy trust, inception schema, FOUNDATION lineage, and post-write readback.

The source test tests/e2e_doctor_chokepoint.rs:622-697 refutes an unconditional no-op interpretation: its second repair is zero-action only after the first repair made the fixture healthy. Any changed control-file hash, template hash, or probe result is a new observed state. L2 therefore adopts L2P-CONDITIONAL-IDEMPOTENCE, not init never writes on its second invocation.

RCH's live doctor exposes the boundary L2 needs: rch doctor --reliability --scope topology,convergence --json --no-self-healing --no-hook-auto-start applies only the requested scopes at rch/src/doctor.rs:1145; its result reports worker topology as pass and repository convergence as explicit unknown when no records exist. L2 must preserve that distinction: a healthy worker topology is not proof that this repository's policy or hooks are trusted.

## Authority / Recovery / Ordering

- `L2R-IDENTITY`: establish repo root, source revision, project id, and host capability provenance first.
- `L2R-TRUST`: existing policy is read-only until trusted_init=true is an explicit decision tied to the repository identity.
- `L2R-BACKUP`: take the target before hash and backup before any write; backup failure refuses the operation.
- `L2R-INIT`: write inception and FOUNDATION artifacts atomically or leave the prior state intact.
- `L2R-REPROBE`: re-run the same control-file and tool probes after writes; mismatch halts.
- `L2R-UNDO`: restore the run's before snapshot by run id, verify hashes, and expose partial recovery as a refusal.
- `L2R-IDEMPOTENCE`: compare complete before-state hashes and probe inputs, not merely invocation count.

## Observability

| ID | Row | Source | Freshness | Known-bad |
|---|---|---|---|---|
| `OBS-L2-IDENTITY` | repo identity | inception.repo_identity plus source revision | per run | path reused for a different repository |
| `OBS-L2-TRUST` | trust decision | trusted_init, policy hashes, decision id | per init attempt | foreign policy changed without opt-in |
| `OBS-L2-WRITE` | backup/write audit | run id, target, before/after hash, backup path | per mutation | target changed with no before hash |
| `OBS-L2-READBACK` | post-init state | inception.json, FOUNDATION S1 row, re-probe report | after every init | write returned success but re-probe fails |

Metric: `MET-L2-INIT-READBACK-FAILURE-RATE` = failed post-init readbacks divided by init attempts, partitioned by trusted_init and state-drift reason. The required floor is 0 for the same-state partition; drift-triggered refusal is not a false success.

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
def repair_writes(previous_state, observed_state):
    return 0 if previous_state == observed_state else 1
assert repair_writes({'policy': 'sha:p'}, {'policy': 'sha:p'}) == 0
assert repair_writes({'policy': 'sha:p'}, {'policy': 'sha:q'}) == 1
print('L2_CONTRACT_VALIDATION PASS trusted_init=anchored inception_foundation=anchored reprobe=anchored conditional_idempotence=state_bound drift_leg=injected production_suite=MISSING')
PY
```

Output:

    L2_CONTRACT_VALIDATION PASS trusted_init=anchored inception_foundation=anchored reprobe=anchored conditional_idempotence=state_bound drift_leg=injected production_suite=MISSING

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

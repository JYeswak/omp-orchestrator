# S1 L0 Install Contract

Bead: `omp-orchestrator-s1w0-l0-install-contract-6gh6`

## Purpose

This contract defines the complete Wave 0 readiness boundary for S1 L0: the install CLI must resolve a supported host triple, verify a prebuilt artifact through SHA-256, minisign, and sigstore policy, stage and atomically publish it with file and parent-directory durability, refuse PATH collisions, detect all ten supported agent families, merge hooks with timestamped backup and restore-on-failure, install skills, emit a per-agent summary and typed observability, prove identity on readback, and uninstall without residue. This file is the build/test matrix, not an implementation claim; S1 cannot advance while any matrix row is MISSING.

## Contract Artifacts

1. Canonical target: `InstallPlan` -> `InstallReport`, proposed at `.omp-orchestrator/work/s1/l0/install-report.json`; the report carries artifact identity, verification, destination, path hits, agents, backups, event reason code, and uninstall result.
2. Runner target: `ompo install --json`, `ompo install --check`, `ompo uninstall --json`; the current identity seam is `crates/installer/src/lib.rs:3-8` and its CLI seam is `crates/installer/src/main.rs:20-39`.
3. Invariant suite target: `tests/l0_install.rs`. No crate or test file is created in this wave; the matrix and named falsifiers are the readiness gate.

## Stable Values

WORKTREE stable-ID set: 16 L0 values plus 9 invariant values = 25. Every one is covered by a build row, test row, or an explicit documentation-only marker in the matrix below.

| ID | Meaning |
|---|---|
| `L0-PLATFORM-TRIPLE` | Supported Darwin/Linux triples and explicit musl-to-gnu fallback. |
| `L0-VERIFY-SHA256` | Mandatory digest verification; tamper refuses. |
| `L0-VERIFY-MINISIGN` | Per-version minisign verification and required-signature policy. |
| `L0-VERIFY-SIGSTORE` | Cosign version, OIDC identity, and local-key policy. |
| `L0-ATOMIC-RENAME` | Complete same-directory staging followed by rename. |
| `L0-DURABILITY-PARENT` | Parent-directory synchronization after rename. |
| `L0-DURABILITY-FULLFSYNC` | macOS `F_FULLFSYNC` durability strengthening. |
| `L0-PATH-COLLISION` | Non-interactive refusal of a conflicting destination/PATH owner. |
| `L0-HOOK-MERGE` | Per-agent hook merge, timestamped backup, and restore-on-failure. |
| `L0-SKILLS` | Skill installation for every detected agent. |
| `L0-UNINSTALL` | Removal of every L0-owned path with no residue. |
| `L0-EVENT` | Lifecycle event with actor, pane, incarnation, outcome, and reason code. |
| `L0-REPORT` | Durable report written before success is exposed. |
| `L0-MONITOR` | Monitor reads report and installed identity. |
| `L0-GATE` | Verification, collision, identity, and observability verdict gate. |
| `L0-METRIC` | `parent_fsync_successes / atomic_rename_attempts`. |
| `INV-L0-TRIPLE` | Unsupported/ambiguous host cannot publish. |
| `INV-L0-CHECKSUM` | One changed artifact byte produces a restrictive refusal. |
| `INV-L0-SIGNATURE` | Required signature failure is restrictive and typed. |
| `INV-L0-COLLISION` | Conflicting destination cannot be overwritten silently. |
| `INV-L0-ATOMIC` | Partial staging never becomes the published destination. |
| `INV-L0-DURABILITY` | File sync, parent sync, and macOS full sync are required. |
| `INV-L0-ROLLBACK` | Failed merge restores exact pre-merge bytes. |
| `INV-L0-IDENTITY` | Installed identity agrees with build/source/process readback. |
| `INV-L0-OBSERVABILITY` | Event, report, monitor row, and gate verdict precede success. |

## Invariant Suite

The future `tests/l0_install.rs` is the executable suite; these invariant rows map directly to test-matrix rows.

- `INV-L0-TRIPLE` — unsupported/ambiguous host cannot publish. Test: `l0_install.rs::platform_triple_matrix` (T01).
- `INV-L0-CHECKSUM` — one changed artifact byte produces a restrictive refusal. Test: `l0_install.rs::tampered_sha256_refuses` (T03).
- `INV-L0-SIGNATURE` — required signature failure is restrictive and typed. Test: `l0_install.rs::required_minisign_missing_refuses` (T04), `l0_install.rs::cosign_below_cve_floor_refuses` (T05), and `l0_install.rs::sigstore_identity_mismatch_refuses` (T06).
- `INV-L0-COLLISION` — conflicting destination cannot be overwritten silently. Test: `l0_install.rs::bare_installer_path_collision_refuses` (T11).
- `INV-L0-ATOMIC` — partial staging never becomes the published destination. Test: `l0_install.rs::staging_failure_leaves_no_destination` (T07) and `l0_install.rs::atomic_publish_renames_complete_artifact` (T08).
- `INV-L0-DURABILITY` — file sync, parent sync, and macOS full sync are required. Test: `l0_install.rs::parent_directory_fsync_is_required` (T09), `l0_install.rs::fullfsync_failure_is_restrictive` (T10), and `l0_install.rs::durability_metric_counts_missing_parent_sync` (T22).
- `INV-L0-ROLLBACK` — failed merge restores exact pre-merge bytes. Test: `l0_install.rs::hook_merge_failure_restores_before_hash` (T15).
- `INV-L0-IDENTITY` — installed identity agrees with build/source/process readback. Test: `l0_install.rs::installed_identity_must_match_readback` (T18).
- `INV-L0-OBSERVABILITY` — event, report, monitor row, and gate verdict precede success. Test: `l0_install.rs::event_and_report_precede_success` (T20), `l0_install.rs::known_bad_legs_name_reason` (T21), and `l0_install.rs::per_agent_summary_is_complete` (T17).

## Laws and Named Proof Tests

Each law has a real test symbol. The test file name is deliberately `l0_install.rs`, matching the build matrix and the acceptance form required by the S1 review bar.

- **`LAW-L0-FAIL-CLOSED`** — no failed verification, collision, merge, identity, or durability operation can produce success. *Test:* `l0_install.rs::failed_precondition_is_restrictive`.
- **`LAW-L0-ATOMIC-DURABLE`** — publication is complete staging, file synchronization, rename, parent-directory `fsync`, and macOS `F_FULLFSYNC`; a missing step is not durable. *Test:* `l0_install.rs::publication_requires_both_sync_boundaries`.
- **`LAW-L0-RESTORE`** — a failed merge restores the exact pre-merge bytes and leaves a cited backup. *Test:* `l0_install.rs::merge_failure_restores_before_hash`.
- **`LAW-L0-REPORT-BEFORE-SUCCESS`** — event and report writes precede the success verdict and share one attempt identity. *Test:* `l0_install.rs::success_requires_event_and_report`.
- **`LAW-L0-OBSERVABLE-REFUSAL`** — every known-bad leg emits a reason code, monitor row, and gate verdict; an exit code alone is insufficient. *Test:* `l0_install.rs::known_bad_legs_name_reason`.
- **`LAW-L0-IDENTITY-READBACK`** — success requires installed identity readback to match the verified build identity. *Test:* `l0_install.rs::installed_identity_must_match_readback`.

## Build Matrix

A build row is one discrete implementation unit that one agent can complete in one pass. WORKTREE matrix totals are 15 build rows, 22 test rows, 6 laws, and 25 stable IDs; `bead key` is the stable title prefix used by the corresponding unclaimed build bead.

| Row | Build item | IDs covered | Re-runnable acceptance |
|---|---|---|---|
| B01 | Platform triple resolver and musl-to-gnu fallback | `L0-PLATFORM-TRIPLE` | Run the platform resolver over the four supported tuples plus an unsupported tuple; expect four typed selections, one explicit fallback, and one refusal. |
| B02 | Artifact staging and bounded verified stream | `INV-L0-ATOMIC` | Run staging with a complete archive and an interrupted stream; expect only the complete stream to produce a same-directory temporary artifact eligible for rename. |
| B03 | SHA-256 verifier | `L0-VERIFY-SHA256`, `INV-L0-CHECKSUM` | Run the verifier against the pinned digest and a one-byte mutation; expect PASS, then `L0_SHA256_REFUSED` with no destination write. |
| B04 | Minisign verifier and required-signature policy | `L0-VERIFY-MINISIGN`, `INV-L0-SIGNATURE` | Run with a valid `.minisig`, absent `.minisig` under `--require-minisign`, and invalid signature; expect valid PASS and typed restrictive failures. |
| B05 | Sigstore/cosign verifier and CVE floor | `L0-VERIFY-SIGSTORE`, `INV-L0-SIGNATURE` | Run with an allowed cosign version/identity and versions below the CVE-2026-22703 floor; expect allowed PASS and `L0_SIGSTORE_REFUSED`. |
| B06 | Atomic destination publication | `L0-ATOMIC-RENAME`, `INV-L0-ATOMIC` | Run publication into a destination while observing the directory; expect a complete artifact only after rename and no partial destination on failure. |
| B07 | File, parent-directory, and macOS full synchronization | `L0-DURABILITY-PARENT`, `L0-DURABILITY-FULLFSYNC`, `INV-L0-DURABILITY`, `L0-METRIC` | Run with successful and injected failed sync calls; expect success only after file sync, parent `fsync`, and macOS `F_FULLFSYNC`, with the metric updated. |
| B08 | Explicit destination and PATH-collision refusal | `L0-PATH-COLLISION`, `INV-L0-COLLISION` | Run with bare `installer` resolving to `/usr/sbin/installer` and with an owned destination; expect collision refusal listing all path hits and no overwrite. |
| B09 | Ten-agent family detection | `L0-EVENT`, `L0-MONITOR` | Run detection with all ten supported agent families and an empty host; expect ten named results and an explicit empty-set error, never inferred success. |
| B10 | Hook merge transaction and backup/restore | `L0-HOOK-MERGE`, `INV-L0-ROLLBACK` | Run a successful merge and a failure after the first mutation; expect timestamped backups and exact restoration on failure. |
| B11 | Skill installation for detected agents | `L0-SKILLS` | Run with the ten-agent detection fixture and one unsupported agent; expect one per-agent skill outcome and no silent omission. |
| B12 | Per-agent summary and durable report | `L0-REPORT`, `INV-L0-OBSERVABILITY` | Run install with mixed `created|merged|already|failed|skipped` outcomes; expect one report containing every agent, backup, path hit, digest, and outcome before success. |
| B13 | Installed identity readback | `INV-L0-IDENTITY` | Run readback against matching and mismatching build id/version/process identity; expect PASS only for the matching tuple. |
| B14 | Uninstall transaction and residue census | `L0-UNINSTALL` | Run uninstall after a full install and after a partial install; expect every L0-owned path absent and the report to list any refused residue. |
| B15 | Lifecycle event, monitor, and gate writer | `L0-EVENT`, `L0-MONITOR`, `L0-GATE`, `INV-L0-OBSERVABILITY` | Run success and each known-bad path; expect one event with a set reason code, one report, one monitor row, and one gate verdict before any success result. |

## Test Matrix and Known-Bad Legs

Every test row has a named function, a grader-runnable acceptance, and an input that MUST turn the test RED. A test without its bad leg is not evidence.

| Row | Named test (`l0_install.rs::<name>`) | IDs/laws | Known-bad input that must turn RED; acceptance |
|---|---|---|---|
| T01 | `l0_install.rs::platform_triple_matrix` | `L0-PLATFORM-TRIPLE`, `INV-L0-TRIPLE` | Unsupported OS/architecture tuple; run the resolver and expect `L0_PLATFORM_REFUSED`, not a guessed target. |
| T02 | `l0_install.rs::musl_fallback_requires_gnu_artifact` | `L0-PLATFORM-TRIPLE` | Musl host with only the gnu fallback artifact absent; expect explicit fallback refusal, not a silently mislabeled binary. |
| T03 | `l0_install.rs::tampered_sha256_refuses` | `L0-VERIFY-SHA256`, `INV-L0-CHECKSUM`, `LAW-L0-FAIL-CLOSED` | Flip one artifact byte; expect `L0_SHA256_REFUSED`, no rename, no success event. |
| T04 | `l0_install.rs::required_minisign_missing_refuses` | `L0-VERIFY-MINISIGN`, `INV-L0-SIGNATURE` | Remove `.minisig` with `--require-minisign`; expect `L0_MINISIGN_REFUSED`, not warning or PASS. |
| T05 | `l0_install.rs::cosign_below_cve_floor_refuses` | `L0-VERIFY-SIGSTORE`, `INV-L0-SIGNATURE` | Supply cosign below the CVE-2026-22703 floor; expect `L0_SIGSTORE_REFUSED` naming the version. |
| T06 | `l0_install.rs::sigstore_identity_mismatch_refuses` | `L0-VERIFY-SIGSTORE`, `INV-L0-SIGNATURE` | Supply a valid signature with a wrong certificate identity; expect restrictive identity refusal. |
| T07 | `l0_install.rs::staging_failure_leaves_no_destination` | `L0-ATOMIC-RENAME`, `INV-L0-ATOMIC` | Interrupt the archive stream before completion; expect no published destination and a failed staging result. |
| T08 | `l0_install.rs::atomic_publish_renames_complete_artifact` | `L0-ATOMIC-RENAME`, `INV-L0-ATOMIC`, `LAW-L0-ATOMIC-DURABLE` | Mutate the temporary artifact after its digest check; expect the publish guard to refuse rather than rename stale bytes. |
| T09 | `l0_install.rs::parent_directory_fsync_is_required` | `L0-DURABILITY-PARENT`, `INV-L0-DURABILITY`, `LAW-L0-ATOMIC-DURABLE` | Inject a parent `fsync` failure after rename; expect `L0_DURABILITY_REFUSED` and no success verdict. |
| T10 | `l0_install.rs::fullfsync_failure_is_restrictive` | `L0-DURABILITY-FULLFSYNC`, `INV-L0-DURABILITY`, `LAW-L0-ATOMIC-DURABLE` | Inject macOS `F_FULLFSYNC` failure; expect restrictive refusal, never a downgraded PASS. |
| T11 | `l0_install.rs::bare_installer_path_collision_refuses` | `L0-PATH-COLLISION`, `INV-L0-COLLISION`, `LAW-L0-FAIL-CLOSED` | Put `/usr/sbin/installer` first on PATH; expect collision refusal naming that path and no overwrite. |
| T12 | `l0_install.rs::ten_agent_detection_is_complete` | `L0-EVENT`, `L0-MONITOR` | Hide one of the ten agent families; expect the missing family in the typed result and a non-success summary. |
| T13 | `l0_install.rs::zero_agents_is_error_not_clean` | `L0-EVENT`, `L0-GATE`, `LAW-L0-OBSERVABLE-REFUSAL` | Detect zero agents; expect an empty-scan ERROR with reason code, not `clean`, PASS, or a zero-agent success report. |
| T14 | `l0_install.rs::hook_backup_matches_mutations` | `L0-HOOK-MERGE`, `INV-L0-ROLLBACK` | Make the number of backups differ from files mutated; expect the invariant to turn RED and identify the mismatch. |
| T15 | `l0_install.rs::hook_merge_failure_restores_before_hash` | `L0-HOOK-MERGE`, `INV-L0-ROLLBACK`, `LAW-L0-RESTORE` | Fail the second hook write; expect byte-identical restoration of the first file and a cited backup. |
| T16 | `l0_install.rs::skill_install_covers_detected_agents` | `L0-SKILLS` | Make one detected agent's skill destination unwritable; expect that agent `failed` and no all-agents success. |
| T17 | `l0_install.rs::per_agent_summary_is_complete` | `L0-REPORT`, `INV-L0-OBSERVABILITY` | Drop one detected agent from the report; expect the completeness assertion to turn RED. |
| T18 | `l0_install.rs::installed_identity_must_match_readback` | `INV-L0-IDENTITY`, `LAW-L0-IDENTITY-READBACK` | Alter installed build id after publication; expect `L0_IDENTITY_MISMATCH` and restrictive exit. |
| T19 | `l0_install.rs::uninstall_leaves_no_owned_paths` | `L0-UNINSTALL` | Leave one owned hook, skill, binary, or report path behind; expect residue census RED with the exact path. |
| T20 | `l0_install.rs::event_and_report_precede_success` | `L0-EVENT`, `L0-REPORT`, `INV-L0-OBSERVABILITY`, `LAW-L0-REPORT-BEFORE-SUCCESS` | Fail event or report write immediately before success; expect success to be refused and no success verdict emitted. |
| T21 | `l0_install.rs::known_bad_legs_name_reason` | `L0-GATE`, `INV-L0-OBSERVABILITY`, `LAW-L0-OBSERVABLE-REFUSAL` | Suppress the reason code or monitor row on a checksum/collision failure; expect the observability law to turn RED. |
| T22 | `l0_install.rs::durability_metric_counts_missing_parent_sync` | `L0-METRIC`, `INV-L0-DURABILITY` | Record an atomic rename without a successful parent sync; expect coverage below `1.0` and a RED metric verdict, never a passing zero. |

## Four L0 Observability Rows

| Row | Writer/reason code | Artifact | Monitor | Gate and known-bad |
|---|---|---|---|---|
| `OBS-L0-EVENT` | Install runner; typed `L0_*` reason code | `LifecycleEvent` with `stage_to=S1.L0` | `ompo journey --expected --json` / portal reader | Empty reason code is RED; T21 must fail. |
| `OBS-L0-REPORT` | Report writer after transaction assembly | `.omp-orchestrator/work/s1/l0/install-report.json` | Report hash and identity readback | Missing/truncated report is `L0_REPORT_MISSING`; T17/T20 must fail. |
| `OBS-L0-MONITOR` | Portal/identity observer | Monitor snapshot with source and timestamp | `ompo portal --check --json` target | Stale or foreign identity is `L0_IDENTITY_MISMATCH`; T18/T21 must fail. |
| `OBS-L0-GATE` | Verification/collision/identity gate | Verdict naming attempt id and reason | Gate output consumed by S1 chain | Tampered checksum, zero agents, or PATH shadow must refuse; T03/T11/T13 must fail. |

**Metric:** `L0_DURABILITY_COVERAGE = parent_fsync_successes / atomic_rename_attempts`. Successful installs require `1.0`; T22 is the known-bad floor test.

## Bead Manifest

Every matrix row has one filed, unclaimed bead. The acceptance in each bead repeats the row's `Run X; expect Y` command and, for every test row, its known-bad input:

Build: `B01=omp-orchestrator-s1-l0-b01-3vro`, `B02=omp-orchestrator-s1-l0-b02-grg4`, `B03=omp-orchestrator-s1-l0-b03-pp2o`, `B04=omp-orchestrator-s1-l0-b04-a6nx`, `B05=omp-orchestrator-s1-l0-b05-qsyg`, `B06=omp-orchestrator-s1-l0-b06-vz1p`, `B07=omp-orchestrator-s1-l0-b07-h6pu`, `B08=omp-orchestrator-s1-l0-b08-u7tm`, `B09=omp-orchestrator-s1-l0-b09-x282`, `B10=omp-orchestrator-s1-l0-b10-3r1g`, `B11=omp-orchestrator-s1-l0-b11-uegf`, `B12=omp-orchestrator-s1-l0-b12-rmr2`, `B13=omp-orchestrator-s1-l0-b13-ebyw`, `B14=omp-orchestrator-s1-l0-b14-f2jh`, `B15=omp-orchestrator-s1-l0-b15-ucvv`.

Test: `T01=omp-orchestrator-s1-l0-t01-o34w`, `T02=omp-orchestrator-s1-l0-t02-mauc`, `T03=omp-orchestrator-s1-l0-t03-26ou`, `T04=omp-orchestrator-s1-l0-t04-kqne`, `T05=omp-orchestrator-s1-l0-t05-xe16`, `T06=omp-orchestrator-s1-l0-t06-3ga8`, `T07=omp-orchestrator-s1-l0-t07-ab3a`, `T08=omp-orchestrator-s1-l0-t08-aqao`, `T09=omp-orchestrator-s1-l0-t09-jx83`, `T10=omp-orchestrator-s1-l0-t10-6idp`, `T11=omp-orchestrator-s1-l0-t11-andf`, `T12=omp-orchestrator-s1-l0-t12-6t0k`, `T13=omp-orchestrator-s1-l0-t13-kvw6`, `T14=omp-orchestrator-s1-l0-t14-pccb`, `T15=omp-orchestrator-s1-l0-t15-tzul`, `T16=omp-orchestrator-s1-l0-t16-m61c`, `T17=omp-orchestrator-s1-l0-t17-sado`, `T18=omp-orchestrator-s1-l0-t18-sk8h`, `T19=omp-orchestrator-s1-l0-t19-shc0`, `T20=omp-orchestrator-s1-l0-t20-i6ki`, `T21=omp-orchestrator-s1-l0-t21-f6si`, `T22=omp-orchestrator-s1-l0-t22-bsfy`.
Law tests: `LAW-FAIL=omp-orchestrator-s1-l0-law-fail-closed-d8v5`, `LAW-ATOMIC=omp-orchestrator-s1-l0-law-atomic-durable-rfs8`, `LAW-RESTORE=omp-orchestrator-s1-l0-law-restore-ylsu`, `LAW-REPORT=omp-orchestrator-s1-l0-law-report-before-success-lxdb`.
## Dispatch Preflight for L0

`FILE → CLAIM → PACKET → ADMISSION → SEND → RECEIPT → ACK → OBSERVE → VERIFY → RECORD`.

- Flow: `br create` files each row with WHAT/WHY/acceptance/bad leg; the future wave claims and assigns it; the packet carries scope, stop, and ACK; fresh admission precedes `ntm --robot-send`; `IDLE→WORKING` is receipt; ACK, observe, non-author verify, then record.
- Must be true: every row has Run X/expect Y acceptance (preventing `DP-GAP-1`), pre-send assignment, fresh two-capture admission, idempotent operation id, refusal-capable ledger, and a dependency on the L0 gate.
- Automated: the gate checks matrix-to-bead-to-acceptance-to-bad-leg coverage; `bv` selects; the dispatcher claims before send and records ACK/receipt/observe evidence; empty/stale scans refuse. Automation never replaces non-author grading.
- L0 escape: calling `install(1)` is mistaken for parent-dir sync proof; checksum warnings, zero-agent scans, and partial hook merges are the other tripwires.

## Validation

C01 FIXED: this check does not read or grep `docs/contracts/s1_l0_install.md`; it reads the independent bead tracker and the L0 gate dependency projection. It proves filed-row acceptance coverage, not document prose or runtime implementation.

```bash
set -eu
rows="$(br list --json | jq '[.issues[] | select((.title|startswith("[L0-B")) or (.title|startswith("[L0-T")) or (.title|startswith("[L0-LAW")))] | length')"
missing="$(br list --json | jq '[.issues[] | select(((.title|startswith("[L0-B")) or (.title|startswith("[L0-T")) or (.title|startswith("[L0-LAW"))) and ((.acceptance_criteria // .acceptance // "") == ""))] | length')"
deps="$(br dep list omp-orchestrator-gate-s1-l0-jtgw --json | jq '[.[] | select(.depends_on_id | startswith("omp-orchestrator-s1-l0-b") or startswith("omp-orchestrator-s1-l0-t") or startswith("omp-orchestrator-s1-l0-law-"))] | length')"
test "$rows" -eq 41
test "$missing" -eq 0
test "$deps" -eq 41
printf 'WORKTREE_EXTERNAL_INPUT L0_EXTERNAL_MATRIX PASS row_beads=%s acceptance_missing=%s gate_matrix_dependencies=%s\n' "$rows" "$missing" "$deps"
```

Pasted output (`WORKTREE`, external tracker inputs):

```text
WORKTREE_EXTERNAL_INPUT L0_EXTERNAL_MATRIX PASS row_beads=41 acceptance_missing=0 gate_matrix_dependencies=41
```
## Rotation Lap 1 Axis (c): named-test denominator

Pane4 extractor `.git/s1_cov.py:16-18` requires a backticked `file.rs::function`. WORKTREE before: `contract.named_test 32/11/21`; after: L0 `named_test 26/22/4`, L0 `stable_id 26/26/0`. Decision: `NAMED_TEST_FORM=converted`; L3-L5 already use this grammar, so changing the extractor would hide the split.
WORKTREE generator output:
```text
S1_REQUIREMENTS=472 COVERED=103 MISSING=369 DOC_ONLY=0
contract.named_test total=59 covered=37 missing=22 doc_only=0
```

WORKTREE delta: total `+27`, covered `+26`, missing `+1`. L0 stable IDs 26/26 and named tests 26/26 are covered by 41 gate edges. The real suite is 9 tests: 8 pass, path-collision refuses by failing RED; 26 contract names collapse 26-to-9. `BUILDABLE=yes: the real suite compiles and runs; L0 correctness is not green because path collision fails and parent-fsync, hook/skill/signature/report/uninstall modules remain absent.`
## Rotation Lap 1 Axis (c): known-bad audit

WORKTREE denominator: 81 test/law rows (`L0=22 L1=22 L2=22 L3=5 L4=5 L5=5`) plus one L0 self-scan leg = 82. Initial WORKTREE `NAMED-AND-PLAUSIBLE=71`, `UNNAMED=9`; L0 itself has no unnamed leg because all six laws map to concrete T rows.

| Row | Target | Bucket | Falsifier/result |
|---|---|---|---|
| C01 | L0 Validation self-scan for own evidence | FIXED (was WOULD_PASS; origin=SELF_SATISFYING) | Validation now reads the external bead tracker and gate dependency projection; no self-grep. |
| C02 | `s1_l2_ecosystem.md` `L2-TEST-REMOTE-A` | WOULD_PASS | Persona A without a remote is an allowed branch; the named condition is accepted, so it is not a bad leg. |

Taxonomy attack: `SELF_SATISFYING` is a strict subset of `NAMED-BUT-WOULD-PASS`; both allow green without subject failure. C01 is now FIXED by external validation; C02 remains the one sibling-owned WOULD_PASS row. Historical buckets were `WOULD_PASS=2`, `SELF_SATISFYING=0`; current `WOULD_PASS=1`, `SELF_SATISFYING=0`, verdict `collapse`.
## Cross-Attack Retained from Wave 0

Static install: `/usr/bin/install` fsyncs the destination fd, not its parent; no `F_FULLFSYNC` path. `dtruss` was SIP-denied, so runtime tracing is UNMEASURED. L3: `ompo` and runner/suite paths absent; object-identity stand-in UNPROVEN.
## Cross-References

- `docs/plan/flow/boxes/S1.toml:10-25,246-275` — L0 inputs/outputs, observability rows, metric, and known-bad requirements.
- `docs/plan/flow/CONTRACT.md:84-113` — zero open rows is necessary only; non-owner refutation, owner refutation, falsifier, and `exists = none` floor.
- `docs/contracts/dispatch_preflight.md:17-34,36-67,73-90` — lifecycle path, `DP-GAP-1`, automation preconditions, and escape routes.
- `crates/installer/src/main.rs:20-39` — current CLI seam.
- `crates/installer/src/lib.rs:3-8` — current four-way identity seam.
- `crates/crate-atom-gate/src/lib.rs:63-65,169-176,464-479` — current tick-path-only SLO trigger that later work must widen.
- `docs/contracts/s1_l3_walkthrough.md:11-13,39-40` — sibling target rejected by the retained cross-attack.
- `mirror:destructive_command_guard/install.sh:805-826,851-858` — platform detection and fallback.
- `mirror:destructive_command_guard/install.sh:946-969` — PATH collision.
- `mirror:destructive_command_guard/install.sh:1265-1295` — SHA-256.
- `mirror:destructive_command_guard/install.sh:1304-1349` — minisign.
- `mirror:destructive_command_guard/install.sh:1385-1445` — sigstore.
- `mirror:destructive_command_guard/install.sh:1708-1712` — staging/atomic install.
- `mirror:destructive_command_guard/install.sh:1797-1798,1862-2024` — backup and agent hook merge.
- `mirror:beads_rust/src/sync/mod.rs:507-509,2425` — parent synchronization after rename.

## NO-CLAIM

This contract does not establish that the full installer (signature, parent-fsync, hooks/skills, report, uninstall), gate, or S1 runtime exists or works. `tests/l0_install.rs` is now a real suite: 8 pass and 1 known RED path-collision test. The matrix and suite do not authorize S2; a green suite would still require non-author grading and the missing production paths.

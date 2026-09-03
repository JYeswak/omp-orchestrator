# S1 L0 Install Contract

Bead: `omp-orchestrator-s1w0-l0-install-contract-6gh6`

## Purpose

This contract defines the L0 human-start install boundary: detect the supported host triple, verify a prebuilt `ompo` artifact, reject destination-name collisions, install atomically, persist the installed identity, merge per-agent hooks and skills with recoverable backups, and emit a typed lifecycle event, durable install report, monitor observation, and gate verdict. It binds fail-closed behavior for tampered artifacts, unsupported hosts, ambiguous PATH ownership, incomplete atomic replacement, failed durability synchronization, and hook-merge rollback; it is a design contract for S1 L0, not an assertion that the current installer already implements every operation.

## Contract Artifacts

1. Canonical artifact: proposed `.omp-orchestrator/work/s1/l0/install-report.json`, the `InstallReport` JSON envelope named by the S1 L0 box. It records host triple, artifact digest and signature verdicts, destination, per-agent merge outcomes, backups, path hits, event reason code, and the installed build identity.
2. Install runner: planned `ompo install --json`, with the existing identity-check surface in `crates/installer/src/main.rs` and `crates/installer/src/lib.rs` as the first consumer seam. The runner is not present as a complete L0 implementation in this wave.
3. Invariant suite: planned `tests/s1_l0_install_contract.rs`; until the crate exists, the in-document invariant suite below is the review oracle and the Validation command checks that the contract names every required law, refusal, observability row, and falsifier.

## Stable Values

Every value below is a stable citation vocabulary for the L0 implementation and its reviewers.

| ID | Value | Property |
|---|---|---|
| `L0-PLATFORM-TRIPLE` | `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-gnu` | Supported host triples; musl-to-gnu fallback is explicit, never guessed. |
| `L0-VERIFY-SHA256` | SHA-256 digest | Mandatory artifact identity check; mismatch refuses installation. |
| `L0-VERIFY-MINISIGN` | Per-version minisign key id | Signature check; `--require-minisign` turns an unavailable/invalid signature into refusal. |
| `L0-VERIFY-SIGSTORE` | Cosign version plus certificate-identity regexp | OIDC identity check, with the documented local-key fallback policy. |
| `L0-ATOMIC-RENAME` | Complete temporary artifact then rename | The destination is never published from a partial stream. |
| `L0-DURABILITY-PARENT` | Parent-directory `fsync` after rename | The rename is not reported durable until the containing directory is synchronized. |
| `L0-DURABILITY-FULLFSYNC` | macOS `F_FULLFSYNC` on the relevant descriptor | The macOS durability strengthening is explicit; inability to complete it is a typed refusal, not a warning. |
| `L0-PATH-COLLISION` | Explicit destination ownership | A bare `installer` name cannot silently resolve to `/usr/sbin/installer`; collision is non-interactive refusal. |
| `L0-HOOK-MERGE` | Backup, merge, restore-on-failure | Per-agent hook/skill changes are reversible and attributable. |
| `L0-EVENT` | `LifecycleEvent` with `stage_to=S1.L0` | Every attempt has a writer, actor, pane, incarnation, outcome, and non-empty reason code. |
| `L0-REPORT` | `InstallReport` | The durable artifact is written before success is exposed. |
| `L0-MONITOR` | `PortalRow` / install identity check | A subsequent monitor reads the report and installed identity, not a guessed PATH state. |
| `L0-GATE` | Checksum, signature, collision, and identity gates | Each gate has a named known-bad leg and refuses the corresponding unsafe state. |
| `L0-METRIC` | `parent_fsync_successes / atomic_rename_attempts` | Durability coverage; target floor is `1.0` for successful installs. |

## Operations

1. **Detect.** Derive the host triple from `uname -s` and `uname -m`. Accept only `L0-PLATFORM-TRIPLE`; use the measured musl-to-gnu fallback only where the upstream rule permits it. Record the selected triple and fallback decision.
2. **Verify.** Require the SHA-256 check. Run the minisign and sigstore checks according to policy, recording version, key/certificate identity, and whether a fallback was used. A digest mismatch is `L0-VERIFY-SHA256` refusal; it is never a warning followed by install.
3. **Resolve destination.** Resolve the explicit `ompo` destination and enumerate PATH hits before writing. If another executable owns the requested bare name, return `L0-PATH-COLLISION` and list the hits; do not prompt or overwrite.
4. **Publish atomically.** Stream the verified archive into a same-directory temporary file, set mode `0755`, synchronize the file, rename the complete file into place, then synchronize the containing directory. On macOS perform `L0-DURABILITY-FULLFSYNC` as well. Any failed required synchronization produces `L0_DURABILITY_REFUSED` and no success report.
5. **Merge.** Merge hooks and skills for each detected agent. Create timestamped backups before each mutation. If any merge fails, restore the affected prior state and emit `L0_HOOK_MERGE_ROLLBACK`; a partial merge is not `merged`.
6. **Prove identity.** Read back the installed artifact identity and compare it with the source HEAD/build id/version/process identity already defined by `crates/installer/src/lib.rs`. A mismatch is restrictive.
7. **Observe and record.** Write the `LifecycleEvent` and `InstallReport`, then expose the monitor row and gate verdict. The event reason code is selected from the typed set below; empty or prose-only reason codes are invalid.

### Typed event reason codes

`L0_INSTALL_SUCCEEDED`, `L0_PLATFORM_REFUSED`, `L0_SHA256_REFUSED`, `L0_MINISIGN_REFUSED`, `L0_SIGSTORE_REFUSED`, `L0_PATH_COLLISION`, `L0_ATOMIC_PUBLISH_REFUSED`, `L0_DURABILITY_REFUSED`, `L0_HOOK_MERGE_ROLLBACK`, and `L0_IDENTITY_MISMATCH`.

## Invariant Suite

The future `tests/s1_l0_install_contract.rs` MUST exercise these invariants against the real runner, not a fixture-only copy:

- **`INV-L0-TRIPLE`** — an unsupported or ambiguous host is a typed refusal and cannot reach publication.
- **`INV-L0-CHECKSUM`** — mutating one artifact byte changes the measured digest and produces `L0_SHA256_REFUSED`; no destination artifact or success event is emitted.
- **`INV-L0-SIGNATURE`** — a required signature failure produces its typed refusal; best-effort mode is recorded distinctly and never relabeled valid.
- **`INV-L0-COLLISION`** — a destination with a conflicting PATH owner refuses non-interactively and lists the collision.
- **`INV-L0-ATOMIC`** — an interrupted or failed stream leaves no published partial destination; only the complete temporary artifact may be renamed.
- **`INV-L0-DURABILITY`** — a successful rename is preceded by file synchronization and followed by parent-directory synchronization; macOS also requires `F_FULLFSYNC` under `L0-DURABILITY-FULLFSYNC`.
- **`INV-L0-ROLLBACK`** — a failure during any agent merge restores the pre-merge bytes and reports every backup and restore action.
- **`INV-L0-IDENTITY`** — the installed artifact's digest/build id/version/process identity agree; one mismatch is restrictive.
- **`INV-L0-OBSERVABILITY`** — every attempt writes one event with a non-empty reason code, one report artifact, one monitor-readable row, and one gate verdict before it can be called successful.

## Laws

- **`LAW-L0-FAIL-CLOSED`** — no failed verification, collision, merge, identity, or durability operation may produce `L0_INSTALL_SUCCEEDED`. *Test:* `tests/s1_l0_install_contract.rs::failed_precondition_is_restrictive`.
- **`LAW-L0-ATOMIC-DURABLE`** — publication is complete temporary write, file synchronization, rename, parent-directory `fsync`, then macOS `F_FULLFSYNC`; a missing step is not durable. *Test:* `tests/s1_l0_install_contract.rs::publication_requires_both_sync_boundaries`.
- **`LAW-L0-RESTORE`** — a failed merge restores the exact pre-merge state and leaves a cited backup. *Test:* `tests/s1_l0_install_contract.rs::merge_failure_restores_before_hash`.
- **`LAW-L0-REPORT-BEFORE-SUCCESS`** — event and report writes precede the success verdict and contain the same attempt identity. *Test:* `tests/s1_l0_install_contract.rs::success_requires_event_and_report`.
- **`LAW-L0-OBSERVABLE-REFUSAL`** — each known-bad gate leg emits its reason code, monitor row, and gate verdict; an exit code without the message is insufficient. *Test:* `tests/s1_l0_install_contract.rs::known_bad_legs_name_reason`.

## Observability Rows

These are the four required L0 rows. A row is not complete unless its writer, durable artifact, monitor, gate, and known-bad leg are named.

| Row | Writer and reason code | Artifact written | Monitor | Gate and known-bad leg |
|---|---|---|---|---|
| `OBS-L0-EVENT` | `ompo install`; one of the typed `L0_*` reason codes, including `L0_INSTALL_SUCCEEDED` | `LifecycleEvent` with `stage_from=HUMAN`, `stage_to=S1.L0`, actor, pane, incarnation, outcome, reason_code, blocker | `ompo portal --check --json` reads the event identity | Empty reason code or event-after-success mutation must be red; require `L0_OBSERVABILITY_REFUSED`. |
| `OBS-L0-ARTIFACT` | Install runner after report assembly | `.omp-orchestrator/work/s1/l0/install-report.json` with digest, signatures, destination, backups, path hits, and identity | Portal row reports artifact path and hash, or `ARTIFACT_MISSING` | Delete/truncate the report after install; monitor must refuse `L0_REPORT_MISSING`, never display healthy. |
| `OBS-L0-MONITOR` | Portal/identity observer after the install transaction | Monitor snapshot records source, installed identity, and observation timestamp | `ompo portal --check --json` plus the existing installer identity-check seam | Point monitor at a different binary or stale report; it must report `L0_IDENTITY_MISMATCH`, not green. |
| `OBS-L0-GATE` | Checksum/signature/path/identity gate at the publication chokepoint | Gate verdict cites the attempt id and failing reason code | The L0 portal row carries the gate verdict | Flip one artifact byte; checksum MUST refuse, not warn. Run with a conflicting PATH owner; collision MUST refuse. |

## Durability Falsifier

The bead's scout reports that `mirror:destructive_command_guard/install.sh:1708-1712` uses a tar stream, temporary file, mode `0755`, and `install -m 0755`, but does not `fsync` the parent directory. The opposing measurement is `mirror:beads_rust/src/sync/mod.rs:507-509`, where `fsync_pinned_parent()` is invoked by the `PostRename` hook at `:2425`.

I attacked the claim against the host's `/usr/bin/install`, rather than adopting either repository's prose. Measured outputs:

- `file -b /usr/bin/install` reports a universal x86_64/arm64e Mach-O.
- `nm -u /usr/bin/install` imports `_fsync`, `_fcntl`, and `_rename`.
- arm64e disassembly shows the replacement path calling `_rename` at `0x10000190c`, reopening the target file at `0x100001928`, and the data-copy path calling `_fsync` at `0x10000237c` with the destination file descriptor. All six `_open` call sites were enumerated; no `_openat` import or parent-directory open path was present. The two `_fcntl` call sites use command `9`, not macOS `F_FULLFSYNC` (`51`).
- A runtime `dtruss` attempt was refused by macOS SIP: `dtrace: failed to initialize dtrace: DTrace requires additional privileges`. That runtime arm is therefore `UNMEASURED`, not negative evidence.

**Result: CONFIRMED for the contract boundary.** macOS `install(1)` has a file-level `fsync` path, but the measured binary does not provide the required post-rename parent-directory synchronization or `F_FULLFSYNC` step. The L0 requirement to perform both is not cargo cult. Keep `L0-DURABILITY-PARENT` and `L0-DURABILITY-FULLFSYNC`; if either cannot complete, refuse success. The remaining no-claim is runtime syscall tracing under SIP, not the static call-path result.

## Metric

**`L0_DURABILITY_COVERAGE = parent_fsync_successes / atomic_rename_attempts`.** Count only attempts with a complete verified artifact and a successful atomic rename. A successful install may be reported only at `1.0`; any lower value is a red L0 gate and must identify the attempts lacking the parent-directory synchronization.

## Validation

One pasteable pre-code contract check. It exercises the in-document invariant suite and verifies that the required runner, stable vocabulary, four observability rows, falsifier, and refusal legs are present; it does not pretend to execute the future installer.

```bash
set -eu
bytes="$(wc -c < docs/contracts/s1_l0_install.md | tr -d ' ')"
ids="$(grep -Eo '`L0-[A-Z0-9-]+`' docs/contracts/s1_l0_install.md | sort -u | wc -l | tr -d ' ')"
rows="$(grep -c '^| `OBS-L0-' docs/contracts/s1_l0_install.md)"
test "$bytes" -le 25600
test "$ids" -ge 5
test "$rows" -eq 4
grep -q '^## Purpose$' docs/contracts/s1_l0_install.md
grep -q '^## Contract Artifacts$' docs/contracts/s1_l0_install.md
grep -q '^## Invariant Suite$' docs/contracts/s1_l0_install.md
grep -q '^## Observability Rows$' docs/contracts/s1_l0_install.md
grep -q '^## Cross-Attack: L3 sibling$' docs/contracts/s1_l0_install.md
grep -q '^## Cross-References$' docs/contracts/s1_l0_install.md
grep -q '^## NO-CLAIM$' docs/contracts/s1_l0_install.md
grep -q '^Bead: `omp-orchestrator-s1w0-l0-install-contract-6gh6`$' docs/contracts/s1_l0_install.md
grep -q 'L0-DURABILITY-PARENT' docs/contracts/s1_l0_install.md
grep -q 'L0-DURABILITY-FULLFSYNC' docs/contracts/s1_l0_install.md
grep -q 'REJECTED (`L3-ARRAY`' docs/contracts/s1_l0_install.md
grep -q 'OMPO=absent' docs/contracts/s1_l0_install.md
test -f crates/installer/src/lib.rs
test -f crates/installer/src/main.rs
printf 'S1_L0_CONTRACT PASS bytes=%s stable_ids=%s observability_rows=%s invariant_suite=declared falsifier=measured_parent_sync\n' "$bytes" "$ids" "$rows"
```

Pasted output after the cross-attack row is added:

```text
S1_L0_CONTRACT PASS bytes=17609 stable_ids=14 observability_rows=4 invariant_suite=declared falsifier=measured_parent_sync
```

## Cross-Attack: L3 sibling

Non-owner review target: `docs/contracts/s1_l3_walkthrough.md:11-13,39-40`. L3 names `ompo start`
and `crates/ompo-start/tests/l3_step_parity.rs` as TARGET/DECLARED and calls `same_object_identity`
the wave-0 executable stand-in for `LAW-L3-SINGLE-ARRAY`.

Command run from this repository root:

```bash
if command -v ompo >/dev/null 2>&1; then echo 'OMPO=present'; else echo 'OMPO=absent'; fi
for p in crates/ompo-start/src/steps.rs crates/ompo-start/tests/l3_step_parity.rs; do
  if test -e "$p"; then echo "$p=present"; else echo "$p=absent"; fi
done
```

Output:

```text
OMPO=absent
crates/ompo-start/src/steps.rs=absent
crates/ompo-start/tests/l3_step_parity.rs=absent
```

**REJECTED (`L3-ARRAY` / `LAW-L3-SINGLE-ARRAY` validation claim):** the named wave-0 stand-in cannot
exercise object identity or prove that two renderers borrow one `Vec<Step>` while both the runner and
the invariant suite are absent. The sibling correctly labels the implementation TARGET and the suite
DECLARED, but calling its future validation a current executable stand-in overstates the evidence. L3
must retain `exists = none` and `UNPROVEN` until the real runner and suite execute against the same
production step source. This is a falsifier attempt and a non-owner rejection, not an opinion.

## Cross-References

- `docs/plan/flow/boxes/S1.toml:10-25` — S1 L0 trigger, `InstallPlan`, `InstallReport`, `LifecycleEvent`, validator, and observability standard.
- `docs/plan/flow/CONTRACT.md:84-113` — retired zero-disagreement rule and the four-clause refutation standard.
- `crates/installer/src/main.rs:20-39` — current installer CLI seam and its explicit check/install verbs.
- `crates/installer/src/lib.rs:3-8` — current four-way identity contract and restrictive mismatch rule.
- `crates/installer/Cargo.toml:1-19` — current installer package and `subprocess-contract` dependency.
- `mirror:destructive_command_guard/install.sh:805-826` — platform triple detection.
- `mirror:destructive_command_guard/install.sh:851-858` — musl-to-gnu fallback.
- `mirror:destructive_command_guard/install.sh:946-969` — PATH collision detection.
- `mirror:destructive_command_guard/install.sh:1265-1295` — SHA-256 verification.
- `mirror:destructive_command_guard/install.sh:1304-1349` — minisign verification.
- `mirror:destructive_command_guard/install.sh:1385-1445` — sigstore verification.
- `mirror:destructive_command_guard/install.sh:1708-1712` — atomic install sequence.
- `mirror:destructive_command_guard/install.sh:1797-1798` — timestamped backup.
- `mirror:destructive_command_guard/install.sh:1862-2024` — per-agent hook merge and restore boundary.
- `mirror:beads_rust/src/sync/mod.rs:507-509` and `:2425` — parent synchronization after rename.

## NO-CLAIM

This document does not establish that `ompo install --json`, the report path, portal monitor, event writer, parent-directory synchronization, or the invariant suite exists in the current binary; those are the L0 build targets. The static `/usr/bin/install` analysis confirms the absence of a parent-directory `fsync`/`F_FULLFSYNC` path in the measured arm64e call graph, but runtime syscall tracing was unavailable under macOS SIP. The document does not establish end-to-end S1 liveness, hook certification, signature-provider availability, or that a successful local contract check means the future installer works. The cross-attack section is required before this contract is committed; a zero-refutation wave is not convergence.

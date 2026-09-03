# Atlas Arc — Current Reality

> Generated from read-only measurement of this checkout. This projection is not canonical; only this output and the Atlas Arc bead were changed.

Generated_at: `2026-09-03T21:42:32.828Z`
Repository_HEAD: `05cdd2adb10bcbc7b5da54fbc9529934146554a8`
S1_coverage: SUSPENDED after `5619f5bedc33ff6fff753a9fba12f0799e96541d feat(s1): add tree-backed coverage kernel [test]`; preserved, not reverted.

## Verdict

Current reality is a partial substrate: 72 Cargo packages, 69 binary targets, 48 packages with a measured caller, and 24 without one. Of 16 declared persisted artifacts, 4 are writer-backed PRESENT, 4 are ABSENT, and 8 are HAND-WRITTEN.

## Four surfaces

| surface | authority | current reading | boundary |
|---|---|---|---|
| Charter | README.md and docs/plan/00-brief.md | mission is explicit: plan → beads → triage → dispatch → verify → close | mission text is not shipped capability |
| Target-state plan | docs/PLAN.md, docs/plan/12-journey.md, SCHEMAS.toml | detailed nine-stage architecture and 16 declarations | many rows are projected, absent, or handwritten |
| Current reality | this generated projection | partial workspace and stale installation/status claims | snapshot-scoped |
| Execution state | .beads/issues.jsonl | 761 parsed rows; closed=104, open=527, in_progress=95, grading=11, tombstone=22, blocked=2 | status and closure require independent evidence |

## 1. Declared artifacts vs disk

PRESENT requires a disk instance plus a measured current writer. HAND-WRITTEN means bytes exist but no production writer was measured. ABSENT means no matching instance. Wildcards are expanded on this host.

| artifact | declared path | instances | state | writer basis | required fields |
|---|---|---|---|---|---|
| `convergence` | `docs/plan/CONVERGENCE.jsonl` | 1 | HAND-WRITTEN | no production writer measured | ["section", "round", "lens", "new_findings", "verdict", "gates_green"] |
| `surface_map` | `docs/plan/SURFACE-MAP.jsonl` | 1 | HAND-WRITTEN | no production writer measured | ["surface", "tool"] |
| `preserved_inventory` | `.flywheel/inventory-artifacts/` | 0 | ABSENT | no disk match | ["artifact", "content_sha256", "source", "captured_at"] |
| `crate_surface` | `OMP-SURFACE-MAP.toml` | 1 | HAND-WRITTEN | no production writer measured | ["classification", "omp_surface", "why"] |
| `tick_state` | `~/.local/state/omp-orchestrator/sessions/<session>/tick-monitor.tsv` | 4 | PRESENT | tick-monitor write path | ["last_tick"] |
| `watch_ledger` | `~/.local/state/omp-orchestrator/sessions/<session>/watch-ledger.jsonl` | 2 | PRESENT | tick-monitor append path | ["tick", "observation"] |
| `grade_evidence` | `/tmp/grade/r<N>-<section>.md` | 0 | ABSENT | no disk match | ["SEVERITY", "SEARCH SPACE"] |
| `beads` | `.beads/issues.jsonl` | 1 | PRESENT | external br-managed | ["id", "title", "status"] |
| `journey_foundation` | `docs/plan/FOUNDATION.jsonl` | 1 | HAND-WRITTEN | no production writer measured | ["schema_version", "stage", "input_refs", "output_refs", "owner", "crates", "gates", "numbers", "known", "unknown", "gaps"] |
| `dispatch_journal` | `docs/plan/DISPATCH.jsonl` | 0 | ABSENT | no disk match | ["ts", "wave", "bead", "targets", "transport", "claim_id", "receipt", "journal_seq"] |
| `human_decisions` | `docs/decisions.jsonl` | 1 | PRESENT | decision-ledger append_line + supervisor caller | ["id", "ts", "question", "decider", "decision", "options_considered", "binds_stages", "supersedes", "review_after", "recorded_by"] |
| `findings_ledger` | `docs/plan/FINDINGS.jsonl` | 1 | HAND-WRITTEN | no production writer measured | ["id", "round", "section", "graded_by", "severity", "finding", "disposition"] |
| `hypotheses` | `docs/plan/HYPOTHESES.jsonl` | 1 | HAND-WRITTEN | no production writer measured | ["id", "prediction", "falsifier", "evidence_scope", "recorded_commit"] |
| `inception_manifest` | `.omp-orchestrator/inception.json` | 0 | ABSENT | no disk match | ["schema_version", "project_id", "repo_identity", "control_files", "host_capabilities", "required_tools", "trust_status"] |
| `numbers` | `NUMBERS.toml` | 1 | HAND-WRITTEN | no production writer measured | ["command", "expect", "appears"] |
| `cross_section_authority` | `docs/plan/CROSS-SECTION-AUTHORITY.jsonl` | 1 | HAND-WRITTEN | no production writer measured | ["schema_version", "record_type"] |

Artifact counts: DECLARED=16 PRESENT=4 ABSENT=4 HAND_WRITTEN=8.

FOUNDATION.jsonl is on disk with 9 rows but no production writer was measured. FINDINGS.jsonl is present but likewise has no measured production writer. A schema row or reader is not a writer receipt.

## 2. Crate BUILT vs WIRED

BUILT means current Cargo metadata includes the package. WIRED means a caller was found outside the crate in non-test production Rust, another crate manifest, .github, or bin. This is static reachability, not runtime usage.

| crate | built | wired | first caller |
|---|---|---|---|
| `ack-spine` | BUILT | WIRED | `crates/omp-orchestrator/Cargo.toml` |
| `ack-stage` | BUILT | WIRED | `crates/omp-orchestrator/Cargo.toml` |
| `admission-reason` | BUILT | UNWIRED | `—` |
| `agent-mail-native` | BUILT | WIRED | `crates/inbox-monitor/Cargo.toml` |
| `asupersync-conformance` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `bead-availability` | BUILT | UNWIRED | `—` |
| `bead-holder` | BUILT | WIRED | `crates/omp-orchestrator/Cargo.toml` |
| `cargo-lane-budget` | BUILT | UNWIRED | `—` |
| `commit-build-fence` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `composer-typed` | BUILT | WIRED | `crates/omp-orchestrator/Cargo.toml` |
| `convergence-stamp` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `crate-atom-gate` | BUILT | WIRED | `crates/no-shell-gate/Cargo.toml` |
| `crate-soundness-verify` | BUILT | UNWIRED | `—` |
| `decision-ledger` | BUILT | WIRED | `crates/omp-orchestrator/Cargo.toml` |
| `dispatch-claim-fence` | BUILT | WIRED | `crates/omp-orchestrator/Cargo.toml` |
| `dispatch-silence-watch` | BUILT | WIRED | `crates/omp-orchestrator/Cargo.toml` |
| `dispatcher-deadman` | BUILT | UNWIRED | `—` |
| `extraction-roster` | BUILT | UNWIRED | `—` |
| `fast-dispatch` | BUILT | WIRED | `crates/loop-coverage/src/lib.rs` |
| `finding` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `finding-dispatch` | BUILT | WIRED | `crates/omp-orchestrator/Cargo.toml` |
| `fleet-composite` | BUILT | UNWIRED | `—` |
| `fleet-monitor` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `fleet-reconcile` | BUILT | WIRED | `crates/refill-idle-panes/Cargo.toml` |
| `fleet-truth` | BUILT | WIRED | `crates/loop-coverage/src/lib.rs` |
| `inbox-monitor` | BUILT | UNWIRED | `—` |
| `installer` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `kernel-bypass-gate` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `kernel-only-operator-hook` | BUILT | UNWIRED | `—` |
| `lifecycle-event` | BUILT | WIRED | `crates/installer/Cargo.toml` |
| `lifecycle-monitor` | BUILT | WIRED | `crates/kernel-only-operator-hook/Cargo.toml` |
| `loop-coverage` | BUILT | WIRED | `crates/ntm-fleet-monitor/Cargo.toml` |
| `loop-driver` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `loop-queue-filter` | BUILT | UNWIRED | `—` |
| `loop-switch` | BUILT | WIRED | `crates/fast-dispatch/Cargo.toml` |
| `loop-tick` | BUILT | UNWIRED | `—` |
| `no-shell-gate` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `ntm-fleet-monitor` | BUILT | WIRED | `crates/fleet-monitor/Cargo.toml` |
| `omp-idle-dispatch` | BUILT | UNWIRED | `—` |
| `omp-inventory-map` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `omp-orchestrator` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `omp-rpc-session` | BUILT | WIRED | `crates/omp-orchestrator/Cargo.toml` |
| `omp-surface-consumption` | BUILT | UNWIRED | `—` |
| `omp-types` | BUILT | WIRED | `crates/omp-inventory-map/src/types_inventory.rs` |
| `oracle-compare` | BUILT | WIRED | `crates/oracle-pane-state-differential/Cargo.toml` |
| `oracle-pane-state-differential` | BUILT | UNWIRED | `—` |
| `orchestration-tick-gate` | BUILT | WIRED | `crates/no-shell-gate/Cargo.toml` |
| `pane-dispatch-fence` | BUILT | UNWIRED | `—` |
| `pane-dispatch-ready` | BUILT | WIRED | `crates/reap-finished-panes/Cargo.toml` |
| `pane-oracle-diff` | BUILT | UNWIRED | `—` |
| `pane-truth` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `path-literal-guard` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `plan-assemble` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `porting-gate` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `pre-delete-citation-check` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `preregistration-gate` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `reap-finished-panes` | BUILT | UNWIRED | `—` |
| `receiver-receipt` | BUILT | WIRED | `crates/ack-spine/Cargo.toml` |
| `refill-idle-panes` | BUILT | UNWIRED | `—` |
| `response-envelope-check` | BUILT | UNWIRED | `—` |
| `s1-coverage` | BUILT | UNWIRED | `—` |
| `scratch-home` | BUILT | WIRED | `crates/ack-spine/Cargo.toml` |
| `sender-identity` | BUILT | WIRED | `crates/omp-orchestrator/Cargo.toml` |
| `silent-success-census` | BUILT | UNWIRED | `—` |
| `staged-build-gate` | BUILT | WIRED | `crates/installer/Cargo.toml` |
| `state-wildcard-lint` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `subprocess-contract` | BUILT | WIRED | `crates/ack-spine/Cargo.toml` |
| `tick-dispatch` | BUILT | UNWIRED | `—` |
| `tick-monitor` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `undrained-pipe-lint` | BUILT | WIRED | `.github/workflows/gate.yml` |
| `verify-dispatch` | BUILT | UNWIRED | `—` |
| `wired-but-inert-guard` | BUILT | UNWIRED | `—` |

Crate counts: CRATES=72 WIRED=48 UNWIRED=24.
Positive control: pattern `subprocess-contract` produced 45 Cargo manifest hits. The control is nonzero.

The dispatch-supplied "finding zero consumers" claim is refuted by current source: `crates/omp-orchestrator/src/main.rs:3601` calls `finding::Finding::recover_pending`, and `:3634-3640` calls `finding_dispatch::finding_for` then `finding.file`. The crate is WIRED; a generic finding grep is still not a substitute for these exact call sites.

## 3. Installed vs in-repo binaries

Scope is the union of 69 Cargo binary targets and 16 declared external binaries from docs/plan/flow/binaries.toml. Backup/quarantine names and non-executable .sh entries are excluded.

| binary | installed | in repo | classification |
|---|---|---|---|
| `ack-spine` | NO | YES | IN_REPO_NOT_INSTALLED |
| `admission-reason` | YES | YES | owned target installed |
| `am` | YES | NO | INSTALLED_NOT_IN_REPO |
| `asupersync` | NO | NO | declared external / absent |
| `asupersync-conformance` | NO | YES | IN_REPO_NOT_INSTALLED |
| `bead-availability` | NO | YES | IN_REPO_NOT_INSTALLED |
| `bead-holder` | NO | YES | IN_REPO_NOT_INSTALLED |
| `br` | YES | NO | INSTALLED_NOT_IN_REPO |
| `bvr` | NO | NO | declared external / absent |
| `cargo-lane-budget` | YES | YES | owned target installed |
| `cass` | YES | NO | INSTALLED_NOT_IN_REPO |
| `commit-build-fence` | NO | YES | IN_REPO_NOT_INSTALLED |
| `composer-typed` | YES | YES | owned target installed |
| `convergence-stamp` | NO | YES | IN_REPO_NOT_INSTALLED |
| `crate-atom-gate` | NO | YES | IN_REPO_NOT_INSTALLED |
| `crate-soundness-verify` | YES | YES | owned target installed |
| `dcg` | YES | NO | INSTALLED_NOT_IN_REPO |
| `decision-ledger` | NO | YES | IN_REPO_NOT_INSTALLED |
| `dispatch-silence-watch` | YES | YES | owned target installed |
| `dispatcher-deadman` | YES | YES | owned target installed |
| `doctor_frankentui` | NO | NO | declared external / absent |
| `dsr` | NO | NO | declared external / absent |
| `extraction-roster` | NO | YES | IN_REPO_NOT_INSTALLED |
| `fast-dispatch` | YES | YES | owned target installed |
| `fleet-composite` | YES | YES | owned target installed |
| `fleet-monitor` | YES | YES | owned target installed |
| `fleet-monitor-diffprobe` | NO | YES | IN_REPO_NOT_INSTALLED |
| `fleet-reconcile` | YES | YES | owned target installed |
| `fleet-truth` | YES | YES | owned target installed |
| `frankenmermaid` | YES | NO | INSTALLED_NOT_IN_REPO |
| `ft` | YES | NO | INSTALLED_NOT_IN_REPO |
| `gate-reachability` | NO | YES | IN_REPO_NOT_INSTALLED |
| `inbox-monitor` | YES | YES | owned target installed |
| `installer` | YES | YES | owned target installed |
| `kernel-bypass-gate` | NO | YES | IN_REPO_NOT_INSTALLED |
| `kernel-only-operator-hook` | YES | YES | owned target installed |
| `lifecycle-event` | NO | YES | IN_REPO_NOT_INSTALLED |
| `lifecycle-monitor` | NO | YES | IN_REPO_NOT_INSTALLED |
| `loop-coverage` | NO | YES | IN_REPO_NOT_INSTALLED |
| `loop-driver` | YES | YES | owned target installed |
| `loop-queue-filter` | YES | YES | owned target installed |
| `loop-switch` | YES | YES | owned target installed |
| `loop-tick` | YES | YES | owned target installed |
| `ms` | YES | NO | INSTALLED_NOT_IN_REPO |
| `no-shell-gate` | NO | YES | IN_REPO_NOT_INSTALLED |
| `ntm` | YES | NO | INSTALLED_NOT_IN_REPO |
| `ntm-activity-fixture-gate` | NO | YES | IN_REPO_NOT_INSTALLED |
| `ntm-fleet-monitor` | YES | YES | owned target installed |
| `omp-idle-dispatch` | YES | YES | owned target installed |
| `omp-inventory-map` | NO | YES | IN_REPO_NOT_INSTALLED |
| `omp-orchestrator` | YES | YES | owned target installed |
| `omp-rpc-session` | NO | YES | IN_REPO_NOT_INSTALLED |
| `omp-surface-consumption` | NO | YES | IN_REPO_NOT_INSTALLED |
| `omp-target-dir` | NO | YES | IN_REPO_NOT_INSTALLED |
| `oracle-pane-state-differential` | YES | YES | owned target installed |
| `orchestration-tick-gate` | NO | YES | IN_REPO_NOT_INSTALLED |
| `pane-dispatch-fence` | YES | YES | owned target installed |
| `pane-dispatch-ready` | YES | YES | owned target installed |
| `pane-oracle-diff` | YES | YES | owned target installed |
| `pane-truth` | YES | YES | owned target installed |
| `path-literal-guard` | NO | YES | IN_REPO_NOT_INSTALLED |
| `plan-assemble` | NO | YES | IN_REPO_NOT_INSTALLED |
| `porting-gate` | NO | YES | IN_REPO_NOT_INSTALLED |
| `pre-commit-gate` | NO | YES | IN_REPO_NOT_INSTALLED |
| `pre-delete-citation-check` | NO | YES | IN_REPO_NOT_INSTALLED |
| `pre-push-gate` | NO | YES | IN_REPO_NOT_INSTALLED |
| `preregistration-gate` | NO | YES | IN_REPO_NOT_INSTALLED |
| `pt-core` | YES | NO | INSTALLED_NOT_IN_REPO |
| `rch` | YES | NO | INSTALLED_NOT_IN_REPO |
| `reap-finished-panes` | YES | YES | owned target installed |
| `receiver-receipt` | YES | YES | owned target installed |
| `refill-idle-panes` | YES | YES | owned target installed |
| `s1-coverage` | NO | YES | IN_REPO_NOT_INSTALLED |
| `sbh` | YES | NO | INSTALLED_NOT_IN_REPO |
| `scratch-home` | NO | YES | IN_REPO_NOT_INSTALLED |
| `silent-success-census` | NO | YES | IN_REPO_NOT_INSTALLED |
| `staged-build-gate` | YES | YES | owned target installed |
| `state-wildcard-lint` | NO | YES | IN_REPO_NOT_INSTALLED |
| `tick-dispatch` | YES | YES | owned target installed |
| `tick-monitor` | YES | YES | owned target installed |
| `ubs` | YES | NO | INSTALLED_NOT_IN_REPO |
| `undrained-pipe-lint` | NO | YES | IN_REPO_NOT_INSTALLED |
| `verify-dispatch` | NO | YES | IN_REPO_NOT_INSTALLED |
| `wired-but-inert-guard` | YES | YES | owned target installed |
| `workspace-load` | NO | YES | IN_REPO_NOT_INSTALLED |

Binary counts: repo_targets=69 declared_external=16 relevant=85 INSTALLED_NOT_IN_REPO=12 IN_REPO_NOT_INSTALLED=36.

Named probes: `pane-truth --version` → pane-truth 0.1.0 build_id=ad31898873c1497cff63e026d6de28b6f80715cb; `inbox-monitor --help` runs and emits usage text from `~/.local/bin/inbox-monitor`; `installer --check` exit=1 reports INSTALLER IDENTITY DRIFT: 4/4 owned binaries disagree with HEAD 05cdd2a.

## 4. Execution state

Atlas bead present=true; status=in_progress. This role does not close it, create repairs, or change the dispatcher.

## 5. Typed contradictions (R4)

Each DEF row separates target assertion, measured evidence, consequence, minimal repair, owner, and verification. Repairs are suspended for this role.

| ID | severity | class | affected IDs | evidence | failure consequence | minimal repair | owner | verification |
|---|---|---|---|---|---|---|---|---|
| `DEF-001` | P0 | declared-boundary-absent | SCHEMAS.inception_manifest; dispatch_journal; grade_evidence; preserved_inventory | Artifact census: four declarations have zero disk matches. | S1/S5 boundaries and grading/provenance evidence cannot be reconstructed. | Create actual writers and receipts through owning beads. | Atlas Arc S1/S5 owners | Rerun census; require disk plus writer/readback evidence. |
| `DEF-002` | P0 | target-state-as-capability | README.md:3-5; docs/PLAN.md:3-4; 72 packages; 69 bins | Metadata and installer probes contradict the one-binary shipped claim. | Partial substrate can be operated as complete. | Caption TARGET-STATE; require clean-machine first-tick proof. | Pane 1 integrator | Clean clone install, help, first tick, identity readback. |
| `DEF-003` | P1 | built-not-wired | admission-reason, bead-availability, cargo-lane-budget, crate-soundness-verify, dispatcher-deadman, extraction-roster, fleet-composite, inbox-monitor, kernel-only-operator-hook, loop-queue-filter, loop-tick, omp-idle-dispatch, omp-surface-consumption, oracle-pane-state-differential, pane-dispatch-fence, pane-oracle-diff, reap-finished-panes, refill-idle-panes, response-envelope-check, s1-coverage, silent-success-census, tick-dispatch, verify-dispatch, wired-but-inert-guard | Conservative caller scan: 48/72 wired; positive control subprocess-contract=45 manifest hits. | Tests can certify lanes no production caller reaches. | Add a real caller or named allowance per row. | Crate owners; pane 1 census | Rerun scoped scan with positive control. |
| `DEF-004` | P1 | artifact-present-without-writer | convergence, surface_map, crate_surface, journey_foundation, findings_ledger, hypotheses, numbers, cross_section_authority | Eight existing declared files have no measured production writer; FOUNDATION.jsonl has 9 rows. | Handwritten projections masquerade as durable state. | Add writer/readback or remove declaration. | SCHEMAS artifact owners | Writer receipt plus byte comparison. |
| `DEF-005` | P1 | installed-identity-drift | ~/.local/bin/omp-orchestrator; tick-monitor; pane-truth; installer |   omp-orchestrator: HEAD=05cdd2adb10bcbc7b5da54fbc9529934146554a8 build_id=a3ab48d3c773115139862b4fd234fc37941a017d version=a3ab48d3c773115139862b4fd234fc37941a017d legs=build_id,version MISMATCH /   tick-monitor: HEAD=05cdd2adb10bcbc7b5da54fbc9529934146554a8 build_id=ad31898873c1497cff63e026d6de28b6f80715cb version=ad31898873c1497cff63e026d6de28b6f80715cb legs=build_id,version MISMATCH /   pane-truth: HEAD=05cdd2adb10bcbc7b5da54fbc9529934146554a8 build_id=ad31898873c1497cff63e026d6de28b6f80715cb version=ad31898873c1497cff63e026d6de28b6f80715cb legs=build_id,version MISMATCH /   installer: HEAD=05cdd2adb10bcbc7b5da54fbc9529934146554a8 build_id=ad31898873c1497cff63e026d6de28b6f80715cb version=ad31898873c1497cff63e026d6de28b6f80715cb legs=build_id,version MISMATCH / INSTALLER IDENTITY DRIFT: 4/4 owned binaries disagree with HEAD 05cdd2a | Installed behavior is not attributable to current HEAD. | Rebuild/install from current repo and rerun identity check. | S8/install owner | Require owned drifted=0. |
| `DEF-006` | P1 | status-table-stale | pane-truth, fleet-truth, fleet-reconcile, oracle-compare, pane-oracle-diff, oracle-pane-state-differential, pane-dispatch-ready, ntm-fleet-monitor, loop-coverage, refill-idle-panes, omp-idle-dispatch, fast-dispatch, tick-dispatch, loop-driver, loop-tick, fleet-monitor, verify-dispatch, dispatcher-deadman, reap-finished-panes, wired-but-inert-guard | AGENTS.md has 20 CONTROL-PLANE rows; all 20 names are current packages; dispatch asserted 22. | Routing uses stale repository-status labels. | Generate table from Cargo metadata. | Pane 1 integrator | Compare every row against metadata. |
| `DEF-007` | P2 | partial-writer-claim | docs/plan/12-journey.md:100; docs/decisions.jsonl; decision-ledger; omp-orchestrator | Source readback found append_line and caller lines 1961-1963; decisions file exists. | Partial S9 mechanism is hidden. | Replace no-writer claim with separate predicates. | S9 decision-ledger owner | Run bounded append and readback. |
| `DEF-008` | P2 | historical-denominator | README.md:17-19; docs/plan/07-installability.md; dispatch 30/68 | Current counts are 72/69 and 48/24; old figures remain literal. | Readiness inherits stale denominators. | Generate every headline count or mark HISTORICAL. | Atlas measurement owner | One-snapshot rerun rejects unproduced integers. |
| `DEF-009` | P3 | carrier-drift | CURRENT-REALITY.md; README.md; reality sidecar | This snapshot was generated by one-shot Bun; no durable generator/sidecar owns the full census. | Reality projection can drift immediately. | Commit generator plus sidecar and freshness gate. | Pane 1 integrator | Generator from clean tree reproduces projection. |

DEFECTS=P0:2,P1:4,P2:2,P3:1.

## 6. Unearned claims

These current-sounding guarantees/proof claims fail their present predicates. Explicit PROJECTED/HISTORICAL target text is not counted solely because it is unfinished.

| ID | source | claim | why unearned now | consequence | minimal repair |
|---|---|---|---|---|---|
| `UC-001` | `README.md:3-5` | One installable binary drives a repo from plan to shipped verified work. | 72 packages, 69 binary targets, 24 unwired packages, and 36 repo binaries not installed. | Readers can treat a partial substrate as shipped. | Generate from clean-machine first-tick evidence. |
| `UC-002` | `README.md:17-19` | Current source has 160 shell scripts, 60,467 lines, and 20 Rust crates. | Current Cargo metadata reports 72 packages. | Scope and progress inherit a stale denominator. | Generate the numbers or label HISTORICAL. |
| `UC-003` | `CLAUDE.md:28-33` | Every crate satisfies all eight clauses, including a wired caller. | 24 of 72 packages have no measured caller. | BUILT can be mistaken for WIRED. | Generate per-crate clause state. |
| `UC-004` | `docs/PLAN.md:3-4` | One installable Rust binary drives the graph to completion and proves every step. | Metadata exposes 69 binary targets; installer --check reports 4/4 MISMATCH. | Target state reads as current capability. | Caption TARGET-STATE and attach a clean install receipt. |
| `UC-005` | `docs/plan/12-journey.md:100` | S9 has no automated writer or lifecycle linkage. | decision-ledger has append_line and omp-orchestrator calls append_request; decisions.jsonl is present. | Existing partial mechanism may be duplicated or ignored. | Split writer, caller, answer, and readback predicates. |
| `UC-006` | `docs/plan/07-installability.md:50,556,574` | Current metadata has 48 binary targets and current arithmetic is 3 installer names. | Current metadata reports 69 repo binary targets and installer drift is nonzero. | Installability decisions use a historical denominator. | Generate target and installed sets together. |

UNEARNED_CLAIMS=6.

## 7. Carrier verdict

A separate markdown review projection is useful, but a hand-maintained one drifts. Durable authority should be a generated README section plus a machine-readable sidecar produced by one freshness-gated measurement command. This requested file is a generated snapshot; the missing durable generator/sidecar is DEF-009.

`CARRIER_VERDICT=generated:one-shot-bun-measurement;durable-generator-and-sidecar-missing`

## NO-CLAIM

- This measures current filesystem/source/binary state; it does not prove runtime correctness, successful end-to-end dispatch, or a clean-machine install.
- WIRED is static caller reachability, not execution or effect.
- PRESENT is file plus writer basis, not schema validity or freshness.
- S1 coverage is suspended, not cancelled; no S2–S9 work was done here.

ARTIFACTS_DECLARED=16 PRESENT=4 ABSENT=4 HAND_WRITTEN=8 CRATES=72 WIRED=48 UNWIRED=24 POSITIVE_CONTROL=subprocess-contract:45 INSTALLED_NOT_IN_REPO=12 IN_REPO_NOT_INSTALLED=36 STALE_STATUS_ROWS=20 DEFECTS=P0:2,P1:4,P2:2,P3:1 UNEARNED_CLAIMS=6 CARRIER_VERDICT=generated:one-shot-bun-measurement;durable-generator-and-sidecar-missing COVERAGE_STOPPED_AT=suspended-after-commit
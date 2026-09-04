# DESIGN_INDEX — omp-orchestrator

**Status:** S4 Round 5 integration complete; four pane-four repairs applied; Round 6 remains required.

**Mode:** `hierarchical`

**Constitution:** `docs/PLAN.md`

**Index authority:** this file is the normative map for delegated companion surfaces and the
cross-companion identifier register. It does not add product scope. Every canonical identifier
below is a deterministic alias for a row, heading, or decision already present at the cited
source authority; `source_id` is retained so the alias can be replayed.

**Source pin:** `/Users/josh/.claude/skills/planning-arc/references/sources.lock.json`, generated
`2026-09-04T16:49:58.989598Z`, SHA-256
`60f44bb0a17e62bd0b9a963827f9ba378d3c5b3c523b236cd644e42fa18949d4`, 134 pinned repositories.
The replay rule is `git clone --filter=blob:none <url> && git checkout <head_sha>` before using
any pinned donor citation.

## 1. Review mode and delegation boundary

`docs/PLAN.md` is the constitution: mission, constraints, architecture direction, cross-section
claims, and the decision to refuse unsupported readiness claims remain there. This index is the
progressive-disclosure boundary. A hierarchical S4 review loads the constitution, this index, and
the companion rows named by the review question. It never treats one contract or one box as the
whole plan.

The monolithic heuristic is not available for this artifact: the measured constitution is
681,088 bytes, above the Grade-B 100–260 KB band. The repository is nevertheless constitution-grade
because the subject is a system others build on, not a single local change, pure research note,
dictated port, or hotfix. The twelve box files are companion stage specifications; they are not
twelve independent plans.

Delegation rule: a companion is normative only for the surface named in its row below. `SCHEMAS.toml`
owns persisted shape; the contract files own their contract and acceptance language; box TOMLs own
stage inputs, outputs, measurements, gaps, and status; `docs/decisions.jsonl` owns recorded human
decision rows. `docs/PLAN.md` owns the cross-surface design claim and must point here instead of
copying those bodies. A conflict is `UNKNOWN` until the owning authority is re-read at its pinned
revision; it is not resolved by whichever prose was read last.

## 2. Normative companion corpus

### 2.1 Persisted artifact schema authority

`SCHEMAS.toml` declares 16 artifact formats. This index does not duplicate required fields; it
names the artifact family and delegates shape checks to that file and its declared reader.

| Artifact declaration | Persisted surface | Reader | Shape authority |
|---|---|---|---|
| `artifacts.convergence` | `docs/plan/CONVERGENCE.jsonl` | `crates/no-shell-gate/tests/convergence.rs` | `SCHEMAS.toml:21-29` |
| `artifacts.surface_map` | `docs/plan/SURFACE-MAP.jsonl` | assembly and census sections | `SCHEMAS.toml:31-38` |
| `artifacts.preserved_inventory` | `.flywheel/inventory-artifacts/` | plan provenance claims and evidence reviewers | `SCHEMAS.toml:40-47` |
| `artifacts.crate_surface` | `OMP-SURFACE-MAP.toml` | `crates/no-shell-gate/tests/wired_lanes.rs` | `SCHEMAS.toml:49-56` |
| `artifacts.tick_state` | session-scoped `tick-monitor.tsv` | `crates/tick-monitor` | `SCHEMAS.toml:58-65` |
| `artifacts.watch_ledger` | session-scoped `watch-ledger.jsonl` | orchestrator observation path | `SCHEMAS.toml:67-74` |
| `artifacts.grade_evidence` | grade evidence records | orchestrator comparing rounds | `SCHEMAS.toml:76-83` |
| `artifacts.beads` | `.beads/issues.jsonl` | `br`, `bv` | `SCHEMAS.toml:85-92` |
| `artifacts.journey_foundation` | `docs/plan/FOUNDATION.jsonl` | plan materializer and stage gates | `SCHEMAS.toml:111-118` |
| `artifacts.dispatch_journal` | `docs/plan/DISPATCH.jsonl` | S6 grading, reap path, and post-mortems | `SCHEMAS.toml:120-127` |
| `artifacts.human_decisions` | `docs/decisions.jsonl` | dispatch packet builder and grading | `SCHEMAS.toml:129-136` |
| `artifacts.findings_ledger` | `docs/plan/FINDINGS.jsonl` | `crates/no-shell-gate/tests/findings_ledger.rs` and integrator | `SCHEMAS.toml:138-145` |
| `artifacts.hypotheses` | `docs/plan/HYPOTHESES.jsonl` | preregistration gate before plan materialization | `SCHEMAS.toml:147-154` |
| `artifacts.inception_manifest` | `.omp-orchestrator/inception.json` | S2 planning foundation and S7 validation | `SCHEMAS.toml:156-163` |
| `artifacts.numbers` | `NUMBERS.toml` | `numbers.rs` and every load-bearing plan claim | `SCHEMAS.toml:165-172` |
| `artifacts.cross_section_authority` | `docs/plan/CROSS-SECTION-AUTHORITY.jsonl` | plan assembler and integrator | `SCHEMAS.toml:174-181` |

### 2.2 Normative contract corpus

All 32 files under `docs/contracts/` are normative companions. Their bodies define the contract
surface; this index only assigns their review grouping.

| Contract | Delegated concern |
|---|---|
| `docs/contracts/ack_spine_contract.md` | acknowledgement authority and evidence spine |
| `docs/contracts/admission_contract.md` | admission decision and refusal boundary |
| `docs/contracts/asupersync_process_grade.md` | bounded subprocess and cancellation grade |
| `docs/contracts/cancellation_contract.md` | cancellation and restrictive outcomes |
| `docs/contracts/claim_strength_contract.md` | claim class and evidence strength |
| `docs/contracts/dispatch_claim_contract.md` | dispatch claim lifecycle |
| `docs/contracts/dispatch_journey_mapping_contract.md` | journey-to-dispatch mapping |
| `docs/contracts/dispatch_preflight.md` | pre-dispatch checks |
| `docs/contracts/degraded_dispatch_policy.md` | typed degraded dispatch |
| `docs/contracts/expectation_registry.md` | expectation and evidence registry |
| `docs/contracts/extraction_eligibility_contract.md` | extraction classification |
| `docs/contracts/finding_contract.md` | finding shape and disposition |
| `docs/contracts/ground_truth_contract.md` | independent ground truth |
| `docs/contracts/hook_design.md` | hook boundary and decision surface |
| `docs/contracts/journey_query.md` | journey query inputs and outputs |
| `docs/contracts/kernel_only_policy.md` | kernel adoption and no handroll rule |
| `docs/contracts/lifecycle_contract.md` | lifecycle transitions |
| `docs/contracts/oracle_comparison_contract.md` | oracle comparison and skew |
| `docs/contracts/orchestration_contract.md` | end-to-end orchestrator lifecycle |
| `docs/contracts/pane_observation_contract.md` | pane observation |
| `docs/contracts/pane_readiness_contract.md` | pane admission readiness |
| `docs/contracts/planning_to_exhaustion.md` | planning exhaustion boundary |
| `docs/contracts/receiver_receipt_contract.md` | receiver delivery evidence |
| `docs/contracts/s1_l0_install.md` | S1 install layer |
| `docs/contracts/s1_l1_doctor.md` | S1 system doctor layer |
| `docs/contracts/s1_l2_ecosystem.md` | S1 ecosystem layer |
| `docs/contracts/s1_l3_walkthrough.md` | S1 walkthrough layer |
| `docs/contracts/s1_l4_liveness.md` | S1 liveness layer |
| `docs/contracts/s1_l5_portal.md` | S1 portal layer |
| `docs/contracts/scratch_home_contract.md` | scratch ownership and reaping |
| `docs/contracts/subprocess_contract.md` | subprocess process-group and pipe discipline |
| `docs/contracts/verification_contract.md` | independent verification |

### 2.3 Stage box authority

All 12 `docs/plan/flow/boxes/*.toml` files are normative companions for stage-level details. They
carry measured fields and explicit gaps; a `PRESENT` row is not a completion claim, and a `MISSING`
row remains a surfaced work item.

| Box | Delegated concern |
|---|---|
| `docs/plan/flow/boxes/S1.toml` | human start and install entry/reference; delegates install, identity, build, and rollback semantics to S8 |
| `docs/plan/flow/boxes/S2.toml` | planning stage |
| `docs/plan/flow/boxes/S3.toml` | plan grading |
| `docs/plan/flow/boxes/S4.toml` | beads graph |
| `docs/plan/flow/boxes/S5a.toml` | execution admission |
| `docs/plan/flow/boxes/S5b.toml` | execution and transport |
| `docs/plan/flow/boxes/S6a.toml` | receipt ACK: independent post-observation carries custody identity |
| `docs/plan/flow/boxes/S6b.toml` | work in progress: tracker-visible stage, block, and silence state |
| `docs/plan/flow/boxes/S6c.toml` | verify grade close: independent evidence to sanctioned bead closure |
| `docs/plan/flow/boxes/S7.toml` | validation |
| `docs/plan/flow/boxes/S8.toml` | sole install, identity, build, and rollback authority; rollback evidence remains unproven until transcript and persisted manifest exist |
| `docs/plan/flow/boxes/S9.toml` | human decision ledger |

**Install precedence:** S8 is the sole normative authority for install, identity, build, and rollback. S1 L0 is the human entry/reference surface and must point to S8; it does not duplicate or override S8's install rules.
**Rollback evidence boundary:** S8 may require a rollback transcript before ship, but the current source records rollback_tests=0 and a non-persisted manifest. Missing rollback artifacts are therefore PROJECTED evidence, not an already-demonstrated gate trip.
`docs/plan/flow/unknowns/DEFECTS.toml`, `DISPOSITIONS.toml`, and `CENSUS.json` are diagnostic
companion evidence. They are not silently promoted into a plan gate: their row status and source
measurement remain part of the cited finding.

## 3. Harvested identifier register

The IDs are materialized from existing numbered requirements, milestone headings, gate rows,
risk rows, and human-decision records. The alias is a durable address; it is not a new requirement,
risk, gate, work package, or decision. Definitions appear exactly once in this register. Cross-
section use belongs in §4.

### 3.1 Source numbering retained; unearned aliases cut

The source requirements remain authoritative as R1–R13 at `docs/plan/00-brief.md:21-80`. The prior requirement aliases were index-only, so they are not retained until a non-index consumer cites them.

### 3.2 Retained invariant provenance

The unnumbered writing-contract bullets at `docs/plan/00-brief.md:569-572` remain source material. Only the already earned provenance marker is retained in the canonical register.

| Canonical ID | Existing authority and invariant |
|---|---|
| `INV-2026` | `docs/plan/02-surface-census.md:14` — retained 2026-08-31 surface-map provenance marker |


### 3.3 Acceptance gates — existing rows normalized, not invented

Business and adoption gates are the existing rows 8–17 in `docs/plan/01-idea.md:415-424`.
Technical gates are the existing eight gate crates in `docs/plan/06-gates.md:31-38`. The final
six rows are the six required gate properties already specified at `docs/plan/06-gates.md:229-234`.

| Canonical ID | Existing source row |
|---|---|
| `GATE-001` | `docs/plan/01-idea.md:415` — reachable population |
| `GATE-002` | `docs/plan/01-idea.md:416` — reachable economics |
| `GATE-003` | `docs/plan/01-idea.md:417` — distribution access |
| `GATE-004` | `docs/plan/01-idea.md:418` — first-value path |
| `GATE-005` | `docs/plan/01-idea.md:419` — paid commitment |
| `GATE-006` | `docs/plan/01-idea.md:420` — unit economics |
| `GATE-007` | `docs/plan/01-idea.md:421` — recurrence and retention |
| `GATE-008` | `docs/plan/01-idea.md:422` — rights, security, and licensing |
| `GATE-009` | `docs/plan/01-idea.md:423` — defensibility and compounding asset |
| `GATE-010` | `docs/plan/01-idea.md:424` — proportionality against substitutes |
| `GATE-011` | `docs/plan/06-gates.md:31` — `no-shell-gate` |
| `GATE-012` | `docs/plan/06-gates.md:32` — `omp-inventory-map` |
| `GATE-013` | `docs/plan/06-gates.md:33` — `undrained-pipe-lint` |
| `GATE-014` | `docs/plan/06-gates.md:34` — `commit-build-fence` |
| `GATE-015` | `docs/plan/06-gates.md:35` — `state-wildcard-lint` |
| `GATE-016` | `docs/plan/06-gates.md:36` — `kernel-bypass-gate` |
| `GATE-017` | `docs/plan/06-gates.md:37` — `pre-delete-citation-check` |
| `GATE-018` | `docs/plan/06-gates.md:38` — `path-literal-guard` |
| `GATE-019` | `docs/plan/06-gates.md:229` — fires on known bad |
| `GATE-020` | `docs/plan/06-gates.md:230` — known-good positive control |
| `GATE-021` | `docs/plan/06-gates.md:231` — mutation |
| `GATE-022` | `docs/plan/06-gates.md:232` — anti-vacuity |
| `GATE-023` | `docs/plan/06-gates.md:233` — floor-raise claim |
| `GATE-024` | `docs/plan/06-gates.md:234` — addressable command and help |

### 3.4 Work packages — existing plan process and milestone headings

`WP-001` and `WP-002` are retained from the S2 gate graph already present in the constitution.
`WP-003` through `WP-009` are deterministic aliases for the seven existing milestone headings;
no milestone body is copied here.

| Canonical ID | Existing source row |
|---|---|
| `WP-001` | `docs/PLAN.md:87` — S2 plan validation repair |
| `WP-002` | `docs/PLAN.md:88` — S4 independent review |
| `WP-003` | `docs/plan/09-milestones.md:45` — M1 shared pane-state seam |
| `WP-004` | `docs/plan/09-milestones.md:76` — M2 graph selection |
| `WP-005` | `docs/plan/09-milestones.md:95` — M3 acknowledgement or typed refusal |
| `WP-006` | `docs/plan/09-milestones.md:119` — M4 completion detection |
| `WP-007` | `docs/plan/09-milestones.md:142` — M5 end-to-end close |
| `WP-008` | `docs/plan/09-milestones.md:163` — M6 foreign-machine execution |
| `WP-009` | `docs/plan/09-milestones.md:215` — M7 unattended refusal accounting |

### 3.5 Source risk rows retained

The six unnumbered risk rows remain authoritative at `docs/plan/01-idea.md:433-438`. Their prior aliases were index-only and are cut; no replacement risk IDs are invented.

### 3.6 Human decisions — every unique ID in the current ledger

The decision kind uses source IDs verbatim. The inclusion rule is **harvest every unique `HD-*` present in `docs/decisions.jsonl` at review time**; duplicate source occurrences remain visible conditions, not additional IDs.

| Kind | Source ID | Existing authority and subject |
|---|---|---|
| DEC | `HD-0001` | `docs/decisions.jsonl` — pending-dispatch fence disposition |
| DEC | `HD-0002` | `docs/decisions.jsonl` — buyer and external-validation decision |
| DEC | `HD-0003` | `docs/decisions.jsonl` — public publishability decision |
| DEC | `HD-0004` | `docs/decisions.jsonl` — worker retirement and respawn decision |
| DEC | `HD-0005` | `docs/decisions.jsonl` — convergence stop condition |
| DEC | `HD-0006` | `docs/decisions.jsonl` — unreconciled finding stop condition |
| DEC | `HD-0007` | `docs/decisions.jsonl` — storage reclaim and RCH decision |
| DEC | `HD-0008` | `docs/decisions.jsonl` — public push decision |
| DEC | `HD-0009` | `docs/decisions.jsonl` — S1/L3 walkthrough substrate |
| DEC | `HD-0010` | `docs/decisions.jsonl` — S1/L4 spawn defaults |
| DEC | `HD-0011` | `docs/decisions.jsonl` — S1 Stop-hook strictness |
| DEC | `HD-0012` | `docs/decisions.jsonl` — OMP profile-hook probe |
| DEC | `HD-0013` | `docs/decisions.jsonl` — macOS SDK and cross-link posture |
| DEC | `HD-0014` | `docs/decisions.jsonl` — S1-first depth versus breadth-before-depth |
| DEC | `HD-0015` | `docs/decisions.jsonl` — conductor refill approval |
| DEC | `HD-0016` | `docs/decisions.jsonl` — current ledger decision row |
| DEC | `HD-0017` | `docs/decisions.jsonl` — current ledger decision row |

## 3.7 Identifier delegation state

Every retained canonical ID has one delegated concern, authority, and consumer state. Range rows cover each individual retained ID; source-only numbering remains at its source authority. This is an honest index map, not a claim that every consumer is implemented.

| Retained ID family | Delegated concern | Delegated to | Consumer state |
|---|---|---|---|
| `GATE-001`–`GATE-024` | business, technical, and six-property gate definitions | the named gate rows and gate owner in `docs/plan/06-gates.md` | real code/bead wiring remains owned by pane %7; not claimed here |
| `WP-001`–`WP-009` | S2/S4 sentinels plus the seven product milestones | the constitution graph and `docs/plan/09-milestones.md` | validator-visible product graph; implementation remains not beads-ready |
| `HD-0001`–`HD-0017` | human decision records | `docs/decisions.jsonl` and the S9 decision owner | all unique ledger IDs harvested at review time; duplicate rows remain source conditions |
| `INV-2026` | retained surface-map provenance | `docs/plan/02-surface-census.md:14` | constitution provenance sentinel |

Delegation completeness remains 32 contract rows + 16 artifact rows + 12 box rows = 60/60. The 31
unearned aliases (the requirement aliases, twelve invariant aliases, and six risk aliases) are cut.
R1–R13, M1–M7, the risk table rows, and the writing-contract bullets remain readable at their source
authorities; no replacement IDs are invented.

## 4. Constitution cross-reference map

This is the reference map the constitution uses during hierarchical review. It tells a reviewer
which authority rows to load; it does not copy their content into a second definition.

| Constitution area | Load these identifiers | Normative companions |
|---|---|---|
| `00-brief` requirements and facts | source R1–R13 | `SCHEMAS.toml`, `docs/plan/FINDINGS.jsonl` |
| `01-idea` thesis and adoption | `GATE-001`–`GATE-010` | `docs/contracts/claim_strength_contract.md`, `docs/contracts/expectation_registry.md` |
| `02-surface-census` | `INV-2026` | `SCHEMAS.toml`, `docs/plan/SURFACE-MAP.jsonl` |
| `03-crates` and process boundaries | `WP-001`–`WP-009` | `docs/contracts/orchestration_contract.md`, `docs/contracts/subprocess_contract.md`, `docs/contracts/cancellation_contract.md` |
| `05-actions` | `GATE-019`–`GATE-024` | `docs/contracts/admission_contract.md`, `docs/contracts/dispatch_claim_contract.md` |
| `06-gates` | `GATE-011`–`GATE-024` | all gate-specific contracts and declared readers |
| `07-installability` | `INV-2026`, `WP-008`, `HD-0013` | `docs/plan/flow/boxes/S8.toml`, S1 entry/reference contracts |
| `08-end-users` | `GATE-003`, `GATE-008` | S1 L2–L5 contracts and boxes |
| `09-milestones` | `WP-003`–`WP-009` | `docs/contracts/verification_contract.md` |
| `10-prior-art` | `INV-2026` | pinned donor sources in `sources.lock.json` |
| `11-lifecycle` | `WP-003`–`WP-009` | `docs/contracts/lifecycle_contract.md` |
| `12-journey` | `WP-001`–`WP-009`, `HD-0001`–`HD-0017` | all 12 stage boxes and journey contracts |

## 5. Product dependency graph

The one canonical graph is the validator-visible table in docs/PLAN.md. This index carries no second edge table or edge count. The source order is M1–M7; the constitution promotes the existing WP-003–WP-009 aliases into that one table. WP-001/WP-002 remain a separate process component in the same table because no source authority says that plan review gates M1.

## 6. Register counts and next-round rule

The retained register is WP=9, INV=1, GATE=24, DEC=17; total IDS_AFTER=51. Thirty-one index-only aliases were cut: the requirement aliases, twelve invariant aliases, and six risk aliases. The seven product aliases are retained because the constitution now references them in the canonical graph.

Decision rule: harvest every unique HD-* present in docs/decisions.jsonl at each review; the current register therefore includes HD-0001 through HD-0017. Duplicate ledger rows are not silently collapsed into evidence.

Round 5 is STRUCTURAL: pane four's four P1 repairs changed the retained ID set, moved the product graph into the validator-visible constitution table, refreshed the decision harvest, and corrected the S8 source authority claim. The decomposition verdict is intentionally not acted on at 9% coverage.

The mechanical validator's exit 0 remains necessary but is not a readiness verdict. Round 6 must re-derive the surviving ID consumers, the single-table DAG, the all-unique decision harvest, and the S8 rollback evidence boundary with fresh eyes.

The Round 4 reviewer must attack the six repaired joins: exact 32=32 path coverage, S1/S8 precedence,
S6 stage alignment, every artifact reader and shape authority, per-ID delegation state, and the distinction
between a projected rollback refusal and a demonstrated gate trip.

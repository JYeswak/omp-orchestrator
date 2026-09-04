# DESIGN_INDEX — omp-orchestrator

**Status:** S4 Round 2 structural integration; not a beads-ready or ship authorization.

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
680,047 bytes, above the Grade-B 100–260 KB band. The repository is nevertheless constitution-grade
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

| Artifact declaration | Persisted surface |
|---|---|
| `artifacts.convergence` | `docs/plan/CONVERGENCE.jsonl` |
| `artifacts.surface_map` | `docs/plan/SURFACE-MAP.jsonl` |
| `artifacts.preserved_inventory` | `.flywheel/inventory-artifacts/` |
| `artifacts.crate_surface` | `OMP-SURFACE-MAP.toml` |
| `artifacts.tick_state` | session-scoped `tick-monitor.tsv` |
| `artifacts.watch_ledger` | session-scoped `watch-ledger.jsonl` |
| `artifacts.grade_evidence` | grade evidence records |
| `artifacts.beads` | `.beads/issues.jsonl` |
| `artifacts.journey_foundation` | `docs/plan/FOUNDATION.jsonl` |
| `artifacts.dispatch_journal` | `docs/plan/DISPATCH.jsonl` |
| `artifacts.human_decisions` | `docs/decisions.jsonl` |
| `artifacts.findings_ledger` | `docs/plan/FINDINGS.jsonl` |
| `artifacts.hypotheses` | `docs/plan/HYPOTHESES.jsonl` |
| `artifacts.inception_manifest` | `.omp-orchestrator/inception.json` |
| `artifacts.numbers` | `NUMBERS.toml` |
| `artifacts.cross_section_authority` | `docs/plan/CROSS-SECTION-AUTHORITY.jsonl` |

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
| `docs/plan/flow/boxes/S1.toml` | human start, install, doctor, ecosystem, walkthrough, swarm, portal |
| `docs/plan/flow/boxes/S2.toml` | planning stage |
| `docs/plan/flow/boxes/S3.toml` | plan grading |
| `docs/plan/flow/boxes/S4.toml` | beads graph |
| `docs/plan/flow/boxes/S5a.toml` | execution admission |
| `docs/plan/flow/boxes/S5b.toml` | execution and transport |
| `docs/plan/flow/boxes/S6a.toml` | work grading |
| `docs/plan/flow/boxes/S6b.toml` | verification |
| `docs/plan/flow/boxes/S6c.toml` | evidence and close |
| `docs/plan/flow/boxes/S7.toml` | validation |
| `docs/plan/flow/boxes/S8.toml` | install, identity, build, rollback |
| `docs/plan/flow/boxes/S9.toml` | human decision ledger |

`docs/plan/flow/unknowns/DEFECTS.toml`, `DISPOSITIONS.toml`, and `CENSUS.json` are diagnostic
companion evidence. They are not silently promoted into a plan gate: their row status and source
measurement remain part of the cited finding.

## 3. Harvested identifier register

The IDs are materialized from existing numbered requirements, milestone headings, gate rows,
risk rows, and human-decision records. The alias is a durable address; it is not a new requirement,
risk, gate, work package, or decision. Definitions appear exactly once in this register. Cross-
section use belongs in §4.

### 3.1 Requirements — 13 existing `R` rows

| Canonical ID | Source ID | Existing authority and subject |
|---|---|---|
| `REQ-001` | `R1` | `docs/plan/00-brief.md:21-24` — A-to-Z journey and milestone completion |
| `REQ-002` | `R2` | `docs/plan/00-brief.md:27-29` — reap before refill and repository hygiene |
| `REQ-003` | `R3` | `docs/plan/00-brief.md:31-35` — one investor-attackable plan |
| `REQ-004` | `R4` | `docs/plan/00-brief.md:38-40` — complete system coverage |
| `REQ-005` | `R5` | `docs/plan/00-brief.md:42` — every OMP surface |
| `REQ-006` | `R6` | `docs/plan/00-brief.md:44` — testing, validation, and gating frameworks |
| `REQ-007` | `R7` | `docs/plan/00-brief.md:47-48` — mirror prior art at every gap |
| `REQ-008` | `R8` | `docs/plan/00-brief.md:50-51` — installability and canonical CLI scoping |
| `REQ-009` | `R9` | `docs/plan/00-brief.md:53-54` — end users and foreign repositories |
| `REQ-010` | `R10` | `docs/plan/00-brief.md:56-60` — idea, action, negative pattern, and SOTA frame |
| `REQ-011` | `R11` | `docs/plan/00-brief.md:62-66` — requirements written before dispatch |
| `REQ-012` | `R12` | `docs/plan/00-brief.md:68-74` — owned economic and risk questions |
| `REQ-013` | `R13` | `docs/plan/00-brief.md:76-80` — idea-to-shipped lifecycle |

### 3.2 Invariants — existing assertions given stable addresses

| Canonical ID | Existing authority and invariant |
|---|---|
| `INV-001` | `docs/plan/00-brief.md:569-570` — every number carries its deriving command |
| `INV-002` | `docs/plan/00-brief.md:571-572` — `MEASURED` and `PROJECTED` do not share a sentence |
| `INV-003` | `docs/plan/00-brief.md:461-462`; `docs/plan/02-surface-census.md:49` — a timeout is not a verdict |
| `INV-004` | `docs/plan/12-journey.md:39` — canonical stage order is S1 → S8 with S9 cross-cutting |
| `INV-005` | `docs/plan/flow/boxes/S1.toml:63-65` — swarm liveness requires three freshness-bearing sources |
| `INV-006` | `docs/plan/06-gates.md:122-124` — empty or unreadable scan input is an error |
| `INV-007` | `docs/plan/06-gates.md:108-110` — mutation goes red and restoration is byte-identical |
| `INV-008` | `docs/plan/06-gates.md:221-223` — gate claims carry ENFORCES, STILL PASSES, and PROVENANCE |
| `INV-009` | `docs/plan/06-gates.md:229-234` — a gate must be addressable by a documented command |
| `INV-010` | `docs/plan/flow/boxes/S9.toml:13,26` — decision append is durable, deduplicated, and restrictive on unreadable input |
| `INV-011` | `SCHEMAS.toml:18-19` — every persisted artifact has a declared required-field row |
| `INV-012` | `docs/plan/12-journey.md:27-35` — each journey stage leaves its named artifact behind |
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

### 3.5 Risks — existing owned risk register

These six rows are the existing owned risk register, not six new risks. The seven milestone-local
`RISK` paragraphs remain local acceptance annotations and are not duplicated into the global register.

| Canonical ID | Existing source row |
|---|---|
| `RISK-001` | `docs/plan/01-idea.md:433` — dispatch blast radius |
| `RISK-002` | `docs/plan/01-idea.md:434` — secrets and tokens |
| `RISK-003` | `docs/plan/01-idea.md:435` — compatibility and upstream drift |
| `RISK-004` | `docs/plan/01-idea.md:436` — licensing and data-use rights |
| `RISK-005` | `docs/plan/01-idea.md:437` — access and distribution |
| `RISK-006` | `docs/plan/01-idea.md:438` — operational failure |

### 3.6 Human decisions — exact existing `HD` IDs, classified as decisions

The decision kind uses the source IDs verbatim. The source ledger currently contains duplicate
occurrences for some IDs; this index counts seven unique IDs and does not erase that source-level
condition. Empty decision fields remain empty evidence, not a filled-in decision.

| Kind | Source ID | Existing authority and subject |
|---|---|---|
| DEC | `HD-0009` | `docs/decisions.jsonl:9` — S1/L3 walkthrough substrate |
| DEC | `HD-0010` | `docs/decisions.jsonl:10` — S1/L4 spawn defaults |
| DEC | `HD-0011` | `docs/decisions.jsonl:11` — S1 Stop-hook strictness |
| DEC | `HD-0012` | `docs/decisions.jsonl:12` — OMP profile-hook probe |
| DEC | `HD-0013` | `docs/decisions.jsonl:13` — macOS SDK and cross-link posture |
| DEC | `HD-0014` | `docs/decisions.jsonl:14` — S1-first depth versus breadth-before-depth |
| DEC | `HD-0015` | `docs/decisions.jsonl:15` — conductor refill approval |

## 4. Constitution cross-reference map

This is the reference map the constitution uses during hierarchical review. It tells a reviewer
which authority rows to load; it does not copy their content into a second definition.

| Constitution area | Load these identifiers | Normative companions |
|---|---|---|
| `00-brief` requirements and facts | `REQ-001`–`REQ-013`, `INV-001`–`INV-003` | `SCHEMAS.toml`, `docs/plan/FINDINGS.jsonl` |
| `01-idea` thesis and adoption | `GATE-001`–`GATE-010`, `RISK-001`–`RISK-006` | `docs/contracts/claim_strength_contract.md`, `docs/contracts/expectation_registry.md` |
| `02-surface-census` | `INV-001`–`INV-003`, `INV-2026` | `SCHEMAS.toml`, `docs/plan/SURFACE-MAP.jsonl` |
| `03-crates` and process boundaries | `INV-003`, `INV-011` | `docs/contracts/subprocess_contract.md`, `docs/contracts/cancellation_contract.md` |
| `05-actions` | `INV-006`–`INV-010` | `docs/contracts/admission_contract.md`, `docs/contracts/dispatch_claim_contract.md` |
| `06-gates` | `GATE-011`–`GATE-024`, `INV-006`–`INV-009` | all gate-specific contracts and declared readers |
| `07-installability` | `INV-011`, `WP-008`, `HD-0013` | `docs/contracts/s1_l0_install.md`, `docs/contracts/s1_l1_doctor.md` |
| `08-end-users` | `REQ-008`, `REQ-009`, `GATE-003`, `GATE-008` | S1 L2–L5 contracts and boxes |
| `09-milestones` | `WP-003`–`WP-009`, `RISK-001`–`RISK-006` | `docs/contracts/verification_contract.md` |
| `10-prior-art` | `REQ-007`, `INV-2026` | pinned donor sources in `sources.lock.json` |
| `11-lifecycle` | `REQ-013`, `INV-004`, `INV-012` | `docs/contracts/lifecycle_contract.md` |
| `12-journey` | `REQ-001`, `REQ-013`, `INV-004`–`INV-010`, `HD-0009`–`HD-0015` | all 12 stage boxes and journey contracts |

## 5. Derived work-package edges

Edges are derived from existing order statements, not guessed from document position. The existing
S2→S4 planning edge is retained. The seven milestone headings explicitly state dependency order,
so their chain contributes six edges. The two components are intentionally not joined until a
source authority states that plan review gates M1.

| Work package | Depends on | Source authority |
|---|---|---|
| `WP-001` | — | existing constitution S2 gate graph |
| `WP-002` | `WP-001` | existing constitution S2 gate graph |
| `WP-003` | — | `docs/plan/09-milestones.md:32-45` — seven milestones ordered by dependency |
| `WP-004` | `WP-003` | `docs/plan/09-milestones.md:32-76` |
| `WP-005` | `WP-004` | `docs/plan/09-milestones.md:32-95` |
| `WP-006` | `WP-005` | `docs/plan/09-milestones.md:32-119` |
| `WP-007` | `WP-006` | `docs/plan/09-milestones.md:32-142` |
| `WP-008` | `WP-007` | `docs/plan/09-milestones.md:32-163` |
| `WP-009` | `WP-008` | `docs/plan/09-milestones.md:32-215` |

Expected unique register counts for this index: `WP=9`, `INV=13`, `GATE=24`, `REQ=13`,
`DEC=7`, `RISK=6`; total `IDS_AFTER=72`. Existing constitution compatibility IDs are
`WP-001`, `WP-002`, and `INV-2026`. No semantic item is invented by this index.
Round 2 delta: 69 unique identifiers became addressable beyond the three existing constitution sentinels; semantic items added = 0.

## 6. Review rule for the next round

Round 2 is `STRUCTURAL`: the review surface changed from an addressless monolith to a constitution
plus an explicit companion boundary and replayable identifier map. The mechanical validator's
`exit 0` remains necessary but is not a readiness verdict. The current state is
`NOT BEADS READY`; the next reviewer must attack whether each alias actually reaches the cited
source, whether the delegated companion really owns the asserted field, and whether the graph can
be materialized without reading the 679 KB constitution again.

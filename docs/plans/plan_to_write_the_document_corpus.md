# plan_to_write_the_document_corpus

Josh, 2026-09-01: *"list every document that we need to write that. lets keep working through all
of the planning in phases."*

Every document type below is **derived from asupersync's corpus, not invented**. Measured
2026-09-01: 457 root-level `docs/*.md` plus 124 in subdirectories = 581, mean 11.7 KB, 1% over
50 KB.

## The document types, by his own naming (457 root docs, final word)

| type | his count | what it is |
|---|---:|---|
| `*_contract.md` | **88** (+4 `_contracts`) | one concern's carrier set, operations, laws, non-coverage |
| `*_matrix.md` | 26 | axes x cases, with every cell resolved |
| `*_inventory.md` | 24 | an enumeration with a denominator |
| `*_policy.md` | 16 | a rule that binds future decisions |
| `*_e2e.md` | 15 | an end-to-end scenario |
| `*_signoff.md` | 10 | a dated acceptance of a bounded claim |
| `*_receipt.md` | 9 | evidence a specific thing happened |
| `*_runbook.md` | 8 | operator steps for a recurring situation |
| `*_audit.md` | 8 | a read-only pass with a verdict |
| `*_report.md` | 8 | a measurement written down |
| `*_schema.md` | 7 | a serialized shape |
| `*_rule.md` | 7 | a single enforceable rule |
| `*_model.md` | 6 | threat/failure/cost model |
| `*_taxonomy.md` | 5 | a controlled vocabulary |
| `*_ledger.md` | 4 | append-only history |
| `*_registry.md` | 3 | the authoritative list of a kind |
| `*_semantics.md` | 3 | formal meaning |
| `*_harness.md` | 3 | how a thing is tested |

Subdirectories: `error_codes/` **45** · `proof/` 15 · `adr/` 13 · `audits/` 13 · `analysis/` 12 ·
`plans/` 11 · `fuzz/` 7 · `design/` 2 · `perf/` 2 · `beads/` 2.

**`error_codes/` at 45 documents is the single biggest omission on our side** — one document per
error family. We have exit codes scattered across 51 crates and not one document.

---

# The manifest — 78 documents, phased

Counts are the plan; each phase re-derives its own denominator on entry.

## PHASE 0 — the type algebra (5 docs) · IN FLIGHT

Detail in `plan_to_pin_the_orchestrator_type_algebra.md`.

1. `docs/contracts/claim_strength_contract.md` — total order, `rank(justifier) >= rank(claim)`
2. `docs/contracts/admission_contract.md` — meet-semilattice, `REFUSE` absorbing
3. `docs/contracts/pane_observation_contract.md` — graded evidence, two-capture dominance
4. `docs/contracts/lifecycle_contract.md` — restrictive terminals that cannot convert to success
5. `docs/contracts/asupersync_process_grade.md` — the recurring grade, re-run each phase boundary

## PHASE 1 — the kernel contracts (9 docs)

The primitives every other crate must route through. `subprocess-contract` and `ack-spine` hold
20+ types with no written contract.

6. `docs/contracts/subprocess_contract.md` — `bounded_output` / `bounded_status` /
   `bounded_passthrough`; deadline, process-GROUP kill, pipe drain, the three outcomes
7. `docs/contracts/cancellation_contract.md` — `&Cx` first, `checkpoint()`, region-owned tasks,
   no detached tasks; **a timeout is not a verdict**
8. `docs/contracts/ack_spine_contract.md` — `TransportAuthority` vs `DeliveryAuthority` vs
   `AckAuthority`: three authorities, one question, and why sender success is not receipt
9. `docs/contracts/receiver_receipt_contract.md` — `ComposerEvidence`, the ≥75 s rule,
   `NonDeliveryEscalation`
10. `docs/contracts/finding_contract.md` — how a gap becomes a filed bead; the unfiled-gap hole
11. `docs/contracts/scratch_home_contract.md` — session-scoped scratch, ownership that dies with
    the thing it owns, `UNKNOWN` is never auto-reaped
12. `docs/contracts/dispatch_claim_contract.md` — file → **claim** → dispatch; a packet naming an
    unclaimed bead is a message, not a dispatch
13. `docs/error_codes/exit_code_registry.md` — every exit code across 51 crates, one row each
14. `docs/contracts/kernel_only_policy.md` — the handroll ban, with the allowlist derivation

## PHASE 2 — the lifecycle stages (11 docs)

One contract per stage of `observe → select → dispatch → verify → reap`, mirroring AGENTS.md's
crate groupings.

15. `docs/contracts/ground_truth_contract.md` — what tmux/ntm each authoritatively answer
16. `docs/contracts/oracle_comparison_contract.md` — claim vs independent oracle; empty oracle is
    an ERROR, never agreement
17. `docs/contracts/pane_readiness_contract.md` — `safe_to_dispatch` is not liveness
18. `docs/contracts/queue_selection_contract.md` — fail-closed selection, epics, in-flight work
19. `docs/contracts/dispatch_contract.md` — admission, fresh-verdict staleness, bounded children
20. `docs/contracts/verification_contract.md` — bead status only, never a pane's self-report
21. `docs/contracts/reaping_contract.md` — an unreaped pane is capacity that vanished silently
22. `docs/contracts/deadman_contract.md` — eligible work that received no packet
23. `docs/matrices/lifecycle_stage_matrix.md` — stage x crate x gate, every cell resolved
24. `docs/inventories/crate_contract_inventory.md` — 51 crates, each with inputs/outputs/exit
    codes. **This is R4, currently 11/51 covered**
25. `docs/contracts/degraded_dispatch_policy.md` — post-mortem M1: what dispatches when admission
    is red

## PHASE 3 — gates and enforcement (12 docs)

26. `docs/matrices/gate_matrix.md` — every gate x {known-bad, known-good, mutation, anti-vacuity,
    reachable trigger}. Replaces the 16-row/8-gate double-count
27. `docs/contracts/gate_authoring_contract.md` — fires-on-known-bad, in-tree specimen, ratchets
    over thresholds
28. `docs/policies/gate_reachability_policy.md` — the census; the third rule of AGENTS.md
29. `docs/contracts/no_shell_contract.md` — the one rule, empty exemption list by design
30. `docs/contracts/ratchet_policy.md` — seed from your OWN scan; the 42-vs-41 slack defect
31. `docs/taxonomies/finding_severity_taxonomy.md` — blocker/major/minor, with decision rules
32. `docs/taxonomies/close_reason_taxonomy.md` — the controlled vocabulary; corpus baseline
33. `docs/contracts/evidence_citation_contract.md` — backticked paths, the harvester's regex
34. `docs/schemas/findings_ledger_schema.md` — promote from `SCHEMAS.toml`
35. `docs/schemas/convergence_schema.md` — round/lens/verdict/pin
36. `docs/rules/self_referential_checker_rule.md` — six instances tonight: a checker whose input
    contains prose about what it checks
37. `docs/policies/shared_checkout_policy.md` — `git commit -- <paths>`; a path-scoped ADD plus a
    bare COMMIT is NOT path-scoped, measured 2026-09-01

## PHASE 4 — the OMP surface (8 docs)

38. `docs/inventories/omp_surface_inventory.md` — 42 RPC methods, 39 subcommands, 71 type entries
39. `docs/matrices/omp_consumption_matrix.md` — per surface: consumed / scraped / unused / not-ours
40. `docs/contracts/omp_rpc_contract.md` — handshake, negotiate, the typed frames
41. `docs/contracts/pane_scraping_policy.md` — when terminal inspection is legitimate (third-party
    panes) and when it is a rewrite of shipped surface
42. `docs/error_codes/omp_refusal_codes.md` — every refusal OMP/NTM can emit
43. `docs/analysis/slash_command_count_analysis.md` — resolve `slash_commands=799` vs
    `expected=136`, the 5.9x R5 discrepancy
44. `docs/contracts/ntm_boundary_contract.md` — what NTM authoritatively answers vs what it
    projects; `total_sessions: 0` with `success: true`
45. `docs/adr/0001-scrape-vs-rpc.md` — the decision, with the cost of each arm

## PHASE 5 — measurement, the whole missing category (13 docs)

Corpus adoption: `fuzz/` 34 repos, `conformance/` 32, `benches/` 28. Ours: **0 / 0 / 0**, and
`fuzz`, `proptest`, `p99` appear **zero times** across our 13 plan sections.

46. `docs/contracts/slo_contract.md` — p50/p95/p99, memory ceilings, `regression_threshold`,
    `safe_mode_trigger`
47. `docs/perf/tick_latency_budget.md` — one tick, end to end
48. `docs/perf/dispatch_throughput_budget.md` — dispatches/minute at N panes
49. `docs/perf/observation_cost_budget.md` — the cost of a full fleet census
50. `docs/harnesses/conformance_harness_contract.md` — **name the external oracle**
51. `docs/harnesses/differential_harness_contract.md` — shell oracle vs Rust port
52. `docs/harnesses/metamorphic_harness_contract.md` — relations that hold under transformation
53. `docs/fuzz/fuzz_target_inventory.md` — every parser and boundary that takes external bytes
54. `docs/fuzz/fuzz_policy.md` — corpus, seeds, CI cadence, what a crash means
55. `docs/harnesses/property_test_inventory.md` — one law per property
56. `docs/models/failure_domain_model.md` — after his `failure_domain_contract.md`
57. `docs/models/threat_model.md` — the fleet's trust boundaries
58. `docs/perf/benchmark_baseline_registry.md` — the committed floor that can only improve

## PHASE 6 — installability and end users (7 docs)

59. `docs/contracts/installability_contract.md` — doctor/health/repair + validate/audit/why
60. `docs/receipts/clean_machine_install_receipt.md` — **R8, absent**
61. `docs/contracts/build_identity_contract.md` — every binary says what it was built from
62. `docs/runbooks/operator_runbook.md` — the recurring situations
63. `docs/runbooks/incident_runbook.md` — the 6-hour idle post-mortem, generalized
64. `docs/receipts/external_repo_first_tick_receipt.md` — **R9, absent**
65. `docs/contracts/end_user_journey_contract.md` — another repo, another machine

## PHASE 7 — governance and closure (13 docs)

66. `docs/registries/requirement_registry.md` — R1–R13, each with its closing artifact's PATH;
    replaces the prose table that drifted in both directions
67. `docs/registries/claim_registry.md` — L1 claims constitution, machine-readable
68. `docs/taxonomies/label_taxonomy.md` — promote `.beads/LABEL-TAXONOMY.md`
69. `docs/beads/bead_shape_contract.md` — WHAT/WHY/ACCEPTANCE, and the argument for front-loaded
    acceptance against the corpus's 16% baseline
70. `docs/policies/phase_arc_policy.md` — phases named once, never re-litigated; sub-epics for
    unforeseen scope
71. `docs/audits/self_audit_runbook.md` — how we grade ourselves
72. `docs/contracts/asupersync_process_grade.md` — recurring, every phase boundary
73–78. `docs/adr/0002..0007` — one ADR per irreversible decision already taken: kernel-only,
    no-shell, Rust-not-shell, beads-as-tracker, scrape-vs-RPC's sibling calls, ratchets-over-
    thresholds

---

## Order, and why this order

Phase 0 and 1 first because **every later document cites those types**. Writing the gate matrix
before `admission_contract.md` means writing the gate matrix twice.

Phase 5 is the one to resist deferring. It is the whole missing category — not an unmet
requirement but an **absent** one — and it is what the corpus treats as standard equipment at
28–34 repos of adoption.

## The rule that keeps this from becoming the thing it replaces

**One concern per document, ~12 KB.** Our current mean is 74 KB with 26% over 50 KB; his is
11.7 KB with 1%. A document that grows past ~25 KB should split, and the split is a finding.

**Convergence writes a successor, never a 24th round.** `proposal_to_X.md` →
`proposal_to_X__after_feedback.md`, both kept, delta readable.

## NO-CLAIM

Seventy-eight is a plan, not a measurement — some will merge, some will split, and some may prove
unnecessary once the type algebra lands. The type taxonomy is derived from one author's corpus
through a daily mirror snapshot; the *categories* are evidenced, the *instances* are my mapping
onto our concerns and are exactly the thing to attack. Writing a document does not discharge a
requirement: a `*_receipt.md` with no run behind it is worse than an absent one, because the
registry will read it as closed.

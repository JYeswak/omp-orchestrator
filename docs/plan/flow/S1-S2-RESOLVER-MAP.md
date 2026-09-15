# S1 → S2 resolver map

Bead: `omp-orchestrator-kyfzy`

Planning-only artifact. `docs/plan/flow/boxes/S2.toml` and `docs/plan/flow/S1-READY.md` are unchanged. This map resolves the nine TOML gap rows and records the supplemental `ompo` readiness row requested by the ruling. It creates no crate, feature bead, or gate.

## Handoff basis

The S1 side is derived from the interfaces, not from the S1 scorecard prose:

- **R10 static half:** the frame/SOTA obligation in `docs/plan/00-brief.md:56-60`, plus the technical dependency floor in `docs/plan/01-idea.md:249-270`.
- **R10 run half:** a retained execution receipt that the gate layer actually ran. The nine passing criteria are readiness inputs, not a substitute for that runtime receipt.
- **S1 output contract:** `docs/plan/flow/maturity0/S1.toml:7-18` declares proposed S1 lifecycle rows, `Inception`, `PortalRow`, and typed decision/not-live conditions. `docs/plan/flow/boxes/S1.toml:19-21` names the kernel input/output and `LifecycleEvent`; its L5 branch requires `inception.json` plus a `FOUNDATION.jsonl` row before S2.
- **S2 hard inputs:** `docs/plan/flow/maturity0/S2.toml:2-16` requires S1 inception/control-file identity, external numbered plan sections, and external `HYPOTHESES.jsonl`.

`S1 delivers it?` is `NO` only where the interface explicitly assigns the input to S2 or marks the S1 capability absent. It is `UNMEASURED` where S1 declares an output but no runtime artifact/readback is present. No YES is inferred from a declaration.

| gap.what (verbatim) | gap.resolves (verbatim) | resolver EXISTS? | consumes from S1 | S1 delivers it? |
|---|---|---|---|---|
| S2 writes no LifecycleEvent row; the plan can change with no journal entry | kxe.8 journal writer + vcd7.1 emit site at plan-assemble's write chokepoint | **EXISTS:** `omp-orchestrator-kxe.8` (`br show` → open); `omp-orchestrator-plan-11-vcd7.1` (`br show` → open); carrier path `crates/plan-assemble` exists | S1 lifecycle handoff: `LifecycleEvent` plus the S1 `Inception`/`PortalRow` identity that the next stage can attach to | **UNMEASURED:** M0 S1 declares proposed lifecycle rows and records, but no runtime S1→S2 event/readback is named by the interface |
| HYPOTHESES.jsonl holds 1 row for a 7969-line plan: preregistration exists as a gate, not as a practice; 235 FINDINGS have no hypothesis they refute or support | plan-check (bcrn.1) PC-n: every section's load-bearing claims map to a hypothesis row or the section is UNPREREGISTERED | **EXISTS:** `omp-orchestrator-plan-09-bcrn.1` (`br show` → open). **NOTHING:** `crates/plan-check` is absent | S1 inception/control-file identity; the hypothesis ledger itself is an explicit external S2 input, not an S1 output | **NO:** M0 S2 lists `docs/plan/HYPOTHESES.jsonl` as external input, and M0 S1 says S1 does not own S2 plan assembly |
| no machine-readable plan items: the only structured form of 01-12 is the hand-made docs/plan/dag/dag-NN.json (567 items) | plan-materialize (n92x) extract stage regenerates them from the sections; differential against the hand manifests | **EXISTS:** `omp-orchestrator-n92x` (`br show` → open). **NOTHING:** `crates/plan-materialize` is absent | R10 static plan sections and their S1 repository identity; materialization is downstream of S2's external numbered sections | **NO:** M0 S2 lists the numbered sections as external and M0 S1 explicitly excludes S2 plan assembly from S1 |
| docs/PLAN.md can be hand-edited; plan-assemble is not the sole writer, so assembly_freshness measures mtimes, not authorship | PLAN.md generated-only: a content-hash manifest of consumed sections (12.67) replaces the mtime key; hand edits refused by the pre-commit gate | **EXISTS:** `omp-orchestrator-plan-12-ibpa.1` (`br show` → open); `docs/plan/dag/dag-12.json` exists and carries the `12.67` entry | S1 repository/control-file identity and the R10 run-half execution receipt needed to bind generated output to a tree | **UNMEASURED:** S1 declares `Inception`/control identity, but no content-hash handoff or runtime R10 receipt is present in the S1 interface |
| no claim row (L1) for 'the plan is assembled from its sections'; no SLO (L2); no fuzz target on the section parser (L5); no wired-caller test (atom 9) | registries/claims.toml row; slo.yaml row for plan-assemble; fuzz/fuzz_targets/plan_assemble_sections.rs; WIRED_CALLERS in plan-assemble | **NOTHING:** `registries/claims.toml`, `slo.yaml`, and `fuzz/fuzz_targets/plan_assemble_sections.rs` are absent; `WIRED_CALLERS` exists in `crates/crate-atom-gate/src/lib.rs`, not in `plan-assemble` | R10 static half: the idea/why/binaries/action-negative/map/design obligations, plus the nine-criterion evidence package | **NO:** M0 S1's non-goals exclude S2 plan assembly and its claim/SLO/fuzz/wired artifacts are S2-owned |
| no Verdict-typed output: plan-assemble exits, it does not return a Verdictlike over Outcome | lf.1.1 Grade/Verdictlike in omp-types, adopted by plan-assemble | **EXISTS:** `omp-orchestrator-jplf.1.1` (`br show` → open); carrier paths `crates/omp-types` and `crates/plan-assemble` exist | S1 typed `Inception`/`PortalRow` identity and R10 static/run outcome vocabulary | **UNMEASURED:** M0 S1 declares typed records as proposed outputs, but no runtime adoption/readback proves the S1 handoff is consumed as a `Grade`/`Verdictlike` |
| S2 -> S3 edge is hand-written (11.5): nothing types 'the plan is ready to grade' | plan-assemble emits PlanAssembled{content_hash, sections[]} consumed by plan-check as its input | **NOTHING:** `br show omp-orchestrator-plan-check` returns `ISSUE_NOT_FOUND`; no target source path for `PlanAssembled` or a plan-check consumer was found. `crates/plan-assemble` exists, but the named transition resolver does not | S1 `LifecycleEvent`/Inception handoff and the R10 run-half receipt that would authorize a typed stage transition | **UNMEASURED:** S1 names the event shape and next-stage handoff, but no actual S1→S2→S3 transition receipt exists |
| no per-box diagram; branches exist only as prose strings | docs/plan/flow/diagrams/S2.mmd from branches; frankenmermaid validate --fail-on warning | **EXISTS:** `docs/plan/flow/diagrams/S2.mmd` exists | R10 static action/negative map and the S1 repository identity from which the next box's branch artifact is resolved | **NO:** the per-box S2 diagram is an S2-owned output; M0 S1 does not list it as an output or owner |
| no hook row: the pre-commit gate is the only hook, and it is per-clone and hand-installed (588v) | hooks_certified.toml row for the pre-commit dispatcher with source_commit + stage; ompo install --hooks writes it | **NOTHING in this target repo:** `hooks_certified.toml`, `docs/hooks_certified.toml`, and `registries/hooks_certified.toml` are absent. The external `/Users/josh/Developer/control-plane/hooks_certified.toml` is not this repo's resolver. `crates/installer` exists, but the S1 box records its four verbs and does not expose a measured `--hooks` resolver | S1 hook identity/install references, `InstallReport`, and the R10 run receipt proving the enforcement layer actually ran | **UNMEASURED:** S1's hook rows are declarative and `certified = "UNATTEMPTED"`; no S1-produced certified registry row or install readback is present |

| **SUPPLEMENTAL** The installable `ompo` artifact has four verbs (`init`, `doctor`, `help`, `capabilities`) but zero callers; `tick`, `observe`, and `dispatch` are deferred by Joshua's ruling. | **ompo caller/wiring census** | **EXISTS:** `ompo` plus `crates/ompo-start` / `crates/ompo-doctor` are named by S1. The supplied live probe reports `ompo capabilities --json` valid and `ompo doctor --repo . --json` status OK, exit 0, 11 probes, journal 1,862 readback lines. | S1's `trigger_target` names `ompo install`, slash start, SessionStart portal check, and launchd run; S1 must deliver installed identity plus a caller/wiring receipt. | **NO + evidence:** supplied census: `crontab 0`, `.git/hooks 0`, `.flywheel 0`, other crates 0, `Command::new(\"ompo\") 0`, with 54 live cron lines and no `ompo`. This is a readiness gap, not a TOML gap row. |

## Resolver counts

Over the **nine S2 TOML gap rows**:

- Resolver `NOTHING`: **3** — gap rows 5, 7, and 9.
- `S1 delivers it? = UNMEASURED`: **5** — gap rows 1, 4, 6, 7, and 9.
- `S1 delivers it? = NO`: **4** — gap rows 2, 3, 5, and 8.
- No YES is asserted. The nonzero NOTHING/UNMEASURED counts are credible because six resolver beads/paths resolve exactly while three compound resolvers are absent in this target, and M0 S1 is still declarative/draft rather than a runtime handoff.

## Negative control

Deliberately absent resolver:

```text
br show omp-orchestrator-resolver-does-not-exist-kyfzy --json
error.code = ISSUE_NOT_FOUND
error.message = Issue not found: omp-orchestrator-resolver-does-not-exist-kyfzy
exit = 3
```

The resolver method reports **NOTHING** with a discriminating error message; it does not silently convert an absent id into a passing empty result.

## Filed gaps

These are recorded here for the frozen planning pass; no feature beads were created:

- **S2 gap 5:** claim/SLO/fuzz/wired-caller carriers for `plan-assemble` are absent.
- **S2 gap 7:** the typed `PlanAssembled`/`plan-check` transition carrier is absent.
- **S2 gap 9:** the target hook registry and automatic `ompo install --hooks` resolver are absent.
- **S1→S2 supplemental:** `ompo` is executable but unwired; its deferred verbs are an explicit Joshua ruling, not an accidental omission.

# Expectation and Performance Registry Contract

Bead: `omp-orchestrator-s1-expectation-perf-registry-xkr6`

## Purpose

This contract defines how a journey stage declares an expected result, how a run records the measured result, how the query computes the delta, and how missing or exceeded expectations become typed verdicts. It separates the existing derived-figure registry from the runtime expectation registry, widens the `crate-atom-gate` SLO applicability trigger from tick-path crates to every journey-stage crate, and binds `ompo journey --expected --delta` to one row shape. It is a Wave 0 design artifact only: no registry writer, gate change, journey query, or SLO implementation is claimed here.

## Contract Artifacts

1. Existing measurement artifact: `NUMBERS.toml`, whose real measured shape is one `[figures.<key>]` table per figure. It remains the derived-number authority unless a later migration explicitly changes its role.
2. Runtime expectation artifact (TARGET, `exists = none` today): `.omp-orchestrator/expectations.toml`, loaded by the journey query using the repository identity and schema version. It is the canonical source for declared stage expectations; it is not a second copy of measured ledgers.
3. Query runner (TARGET): `ompo journey <bead-id|box-id|run-id|pane-id> --expected --delta --json`.
4. Invariant suite (DECLARED this wave, not compiled): `tests/expectation_registry_contract.rs`. Until the implementation exists, the Validation command below is the document-level stand-in and must not be read as runtime proof.

## Existing NUMBERS Shape

The bead's command was re-run against the file itself:

```text
NUMBERS.toml: 234 lines, 17,859 bytes
[figures.<key>] tables: 34
[[array-of-tables] headers: 0
field occurrences: command=34, expect=34, note=34, appears=29, zero_is_real=2
```

The actual row is therefore a dotted single-table key, not an array-of-tables row:

```toml
[figures.workspace_crates]
command = "..."
expect = "LIVE"
appears = ["..."]
note = "..."
```

`command`, `expect`, and `note` are present in all 34 measured rows. `appears` is present in 29 and `zero_is_real` in 2. `expect = "LIVE"` means that the command result is intentionally volatile; it is not a numeric performance expectation. A line count or an `expect` string alone cannot supply a stage SLO.

To make `NUMBERS.toml` an expectation source, every row would additionally need a typed `expectation_id`, `scope`, `metric`, `unit`, `expected_value` or `expected_duration`, `threshold`, `measurement_window`, `source_command`, `reader`, and `measured_at` policy, with a schema version and duplicate-key refusal. That is a migration, not the current shape. The safer decision is below: retain `NUMBERS.toml` for derived figures and put runtime expectations in a repository-local typed registry.

## Stable Values

| ID | Value | Property |
|---|---|---|
| `EXP-NUMBERS-SHAPE` | `[figures.<key>]` | Existing 34-row single-table measurement shape; zero `[[...]]` rows. |
| `EXP-RUNTIME-HOME` | `.omp-orchestrator/expectations.toml` | Runtime-readable canonical expectation source; `NUMBERS.toml` is not the runtime source. |
| `EXP-STAGE-ROW` | `journey_stage`, `expectation_id`, `metric`, `unit` | Join identity for one expected stage result. |
| `EXP-DURATION-ROW` | `expected_duration`, `measured_duration`, `threshold`, `verdict` | Required materialized query row fields. |
| `EXP-MISSING` | `MISSING_EXPECTATION` | Known absence of a matching expectation; never `PASS`, `RED`, or silent omission. |
| `EXP-UNKNOWN` | `UNKNOWN` | Expectation exists but measurement is absent or unobservable. |
| `EXP-RED-ABSORBING` | `RED` | A known numeric SLO exceedance remains journey `RED` even when all steps succeeded. |
| `EXP-SLO-TRIGGER` | `on_tick_path || journey_stage.is_some()` | Part 7 applies to journey stages as well as tick-path crates. |
| `EXP-DELTA` | `measured_duration - expected_duration - threshold` | Signed performance delta in the declared unit. |
| `EXP-QUERY-ENVELOPE` | `schema_id`, `schema_version`, `content_hash`, `selector`, `stages`, `overall_verdict`, `refusals` | Stable `ompo journey` response envelope. |
| `EXP-OBSERVED-AT` | `observed_at` plus `source_command` | Every measured value carries provenance and time. |

## Registry Row and Materialized Result

The runtime registry contains declared expectations only. A declaration is one `[[expectation]]` row:

```toml
schema_id = "omp-orchestrator.expectations"
schema_version = 1

[[expectation]]
id = "s1.l0.install.wall_duration"
scope = "journey_stage"
stage = "S1.L0"
metric = "wall_duration"
unit = "ms"
expected_duration = 30000
threshold = 5000
source_command = "ompo install --json"
owner = "installer"
```

The journey query joins that declaration to a measured stage event and materializes exactly this row shape:

```json
{
  "journey_stage": "S1.L0",
  "expectation_id": "s1.l0.install.wall_duration",
  "metric": "wall_duration",
  "unit": "ms",
  "expected_duration": 30000,
  "measured_duration": 28100,
  "threshold": 5000,
  "delta": -6900,
  "verdict": "PASS",
  "source_command": "ompo install --json",
  "observed_at": "2026-09-03T00:00:00Z"
}
```

`measured_duration` is emitted by the run ledger, not hand-entered in a static expectation file. If no declaration matches the stage, the materialized row has `expected_duration = null`, `measured_duration = null` or its available measured value, `threshold = null`, `verdict = "MISSING_EXPECTATION"`, and a refusal/detail naming the stage. If a declaration exists but no trustworthy measured duration exists, the verdict is `UNKNOWN`. A malformed, duplicate, ambiguous, or unit-incompatible row is `REFUSED`, not `UNKNOWN`.

### Verdict precedence

1. `RED` if any stage has numeric `measured_duration > expected_duration + threshold`.
2. Otherwise `MISSING_EXPECTATION` if any required stage has no declaration.
3. Otherwise `UNKNOWN` if any declared stage lacks a trustworthy measurement.
4. `PASS` only when every required stage has a declaration and measurement and no stage exceeds its threshold.
5. `REFUSED` for malformed registry or query identity; it never downgrades into a plausible performance result.

`RED` is absorbing at the journey level. Thus a stage can report every step as successful while the stage and journey report `RED` for an SLO breach. Step outcome and stage performance are separate dimensions; one must not overwrite the other. A missing expectation is not an SLO breach because the comparison cannot be performed.

## SLO Trigger Widening

`crate-atom-gate` currently defines Part 7 as `Slo = 7` at `crates/crate-atom-gate/src/lib.rs:63-65`, stores `on_tick_path` at `:174-176`, and returns `NotApplicable` for every fact with `on_tick_path == false` at `:464-468`. The widened contract is:

```text
slo_applies = facts.on_tick_path || facts.journey_stage.is_some()
```

The existing tick-path behavior remains. The new known-good leg is `on_tick_path=false` and `journey_stage=None`, which remains `NotApplicable`. The required known-bad leg is `on_tick_path=false`, `journey_stage=Some("S1.L0")`, and `slo_rows={}`; the gate must return a typed `SLO_MISSING` refusal rather than `NotApplicable`, and the overall gate must be nonzero. The future invariant suite names this leg `journey_stage_without_slo_is_refused`. No `crate-atom-gate` source edit is part of this wave.

## Query Contract

`ompo journey <bead-id|box-id|run-id|pane-id> --expected --delta --json` reads the runtime expectation registry by repository identity, joins measured stage events by the typed `journey_key`, and emits the `EXP-QUERY-ENVELOPE`. `--expected` adds the declared and materialized expectation rows. `--delta` adds signed deltas and the verdict calculation; it must not silently omit stages with no declaration or measurement. The selector is polymorphic but the response always retains the resolved bead, box, run, pane, and stage identities.

The query must preserve the source command for both sides of every comparison: the registry declaration's `source_command` and the measured ledger event's `source_command`. It must return a typed refusal when the selector resolves to multiple incompatible runs or when two registry rows claim the same `expectation_id` with different values.

## Invariant Suite

The future `tests/expectation_registry_contract.rs` MUST exercise the real registry/query and gate path, not only parse fixtures:

- **`INV-EXP-NUMBERS-SHAPE`** — the existing dotted table shape is measured explicitly; zero array rows cannot be mistaken for a row count.
- **`INV-EXP-MISSING`** — a required journey stage without a declaration returns `MISSING_EXPECTATION`, not `PASS`, `RED`, or omission.
- **`INV-EXP-UNKNOWN`** — a declared stage without a trustworthy measurement returns `UNKNOWN`, not `PASS`.
- **`INV-EXP-RED`** — a numeric stage over its threshold returns `RED` even when every step outcome is successful.
- **`INV-EXP-RED-ABSORBING`** — one known SLO breach keeps the journey `RED`; later `PASS` rows cannot erase it.
- **`INV-EXP-DELTA`** — delta uses the declared unit and the exact formula `measured - expected - threshold`.
- **`INV-SLO-JOURNEY-TRIGGER`** — a journey-stage crate with no SLO row is refused; a non-tick, non-journey crate remains `NotApplicable`.
- **`INV-EXP-DUPLICATE`** — duplicate or incompatible expectation IDs are refused before query output.
- **`INV-EXP-PROVENANCE`** — every expected and measured value carries its source command and observation time.
- **`INV-EXP-ENVELOPE`** — `--expected` and `--delta` preserve the stable envelope and include missing/unknown rows.

## Laws

- **`LAW-EXP-NO-SILENT-MISSING`** — absence of an expectation is a named restrictive result. *Test:* `tests/expectation_registry_contract.rs::missing_expectation_is_not_success`.
- **`LAW-EXP-STAGE-RED`** — a known stage SLO breach is `RED` independently of step success. *Test:* `tests/expectation_registry_contract.rs::successful_steps_do_not_mask_slo_red`.
- **`LAW-EXP-PASS-FLOOR`** — `PASS` requires complete expected and measured coverage with no threshold breach. *Test:* `tests/expectation_registry_contract.rs::pass_requires_complete_stage_rows`.
- **`LAW-EXP-TRIGGER-WIDENED`** — journey-stage applicability cannot be classified `NotApplicable` merely because the crate is not on a tick path. *Test:* `tests/expectation_registry_contract.rs::journey_stage_without_slo_is_refused`.
- **`LAW-EXP-ONE-HOME`** — the running query reads one typed runtime expectation source; derived-number prose is not silently promoted to runtime truth. *Test:* `tests/expectation_registry_contract.rs::runtime_registry_identity_is_explicit`.

## Observability and Metric

| Row | Writer | Artifact | Monitor | Gate and known-bad leg |
|---|---|---|---|---|
| `OBS-EXP-DECLARATION` | Registry writer | `.omp-orchestrator/expectations.toml` | `ompo journey --expected --json` | Delete a required row; output `MISSING_EXPECTATION`, never `PASS`. |
| `OBS-EXP-MEASUREMENT` | Journey-stage completion writer | Stage event with `measured_duration`, `source_command`, `observed_at` | `ompo journey --delta --json` | Remove measurement; output `UNKNOWN`, never zero or `PASS`. |
| `OBS-EXP-VERDICT` | Query verdict reducer | Envelope with stage rows and `overall_verdict` | Portal/journey monitor | Set measured duration above threshold while all steps succeed; output `RED`. |
| `OBS-SLO-TRIGGER` | `crate-atom-gate` after trigger widening | Gate row naming `journey_stage` and `SLO_MISSING` | Gate report | Journey-stage fact with empty `slo_rows`; nonzero refusal, not `NotApplicable`. |

**Metric:** `PERF_SLO_DELTA_MS = measured_duration - expected_duration - threshold`. It is signed per stage, uses the row's declared unit, and is only numeric when both expected and measured durations are present. Missing and unknown rows are categorical and must never be coerced to zero.

## Attacks

### Attack 1: successful steps versus an exceeded SLO

Owner assertion attacked: a journey is `RED` when a stage exceeds its SLO even if every step succeeded. The attack command checked whether today's S1 observability rows actually carry the operands needed to make that decision:

```text
observability_rows=6 metric_rows=6 explicit_expectation_fields=0
```

**Result: qualified, not blindly adopted.** The numeric assertion survives when `expected_duration`, `measured_duration`, and `threshold` are all present; `RED` is absorbing at the journey level. The unqualified version is rejected because today's rows cannot perform the comparison. Missing expectation must be `MISSING_EXPECTATION`, and missing measurement must be `UNKNOWN`. This is the required distinction between a breached SLO and an uninstrumented stage.

### Attack 2: `NUMBERS.toml` as runtime home

Owner assertion attacked: `NUMBERS.toml` is the right home for expectations. The file-shape and runtime-consumer probes returned:

```text
figure_tables=34
array_tables=0
field_names=appears command expect note zero_is_real
runtime_expected_refs=0
```

**Result: rejected as the sole runtime home.** `NUMBERS.toml` is a useful derived-figure registry with command provenance, but its `expect` field is overloaded with `LIVE`/literal figure semantics, its row shape is not expectation-specific, and no production crate currently reads an expectation query surface from it. The runtime canonical source is the typed repository-local `.omp-orchestrator/expectations.toml`; `NUMBERS.toml` may gain an `expectation_id` reference later, but duplicate expectation values are forbidden.

## Validation

One pasteable pre-code contract check. It measures the live `NUMBERS.toml` shape, checks this contract's invariant vocabulary and query fields, and confirms that the source seam being widened exists. It does not build or execute the future registry, journey query, or gate.

```bash
set -eu
bytes="$(wc -c < docs/contracts/expectation_registry.md | tr -d ' ')"
ids="$(grep -Eo '`EXP-[A-Z0-9-]+`' docs/contracts/expectation_registry.md | sort -u | wc -l | tr -d ' ')"
numbers_tables="$(grep -c '^\[figures\.' NUMBERS.toml)"
array_tables="$(grep -c '^\[\[' NUMBERS.toml || true)"
attacks="$(grep -c '^### Attack ' docs/contracts/expectation_registry.md)"
test "$bytes" -le 25600
test "$ids" -ge 5
test "$numbers_tables" -eq 34
test "$array_tables" -eq 0
test "$attacks" -eq 2
grep -q '^## Purpose$' docs/contracts/expectation_registry.md
grep -q '^## Contract Artifacts$' docs/contracts/expectation_registry.md
grep -q '^## Stable Values$' docs/contracts/expectation_registry.md
grep -q '^## Invariant Suite$' docs/contracts/expectation_registry.md
grep -q '^## Laws$' docs/contracts/expectation_registry.md
grep -q '^## Observability and Metric$' docs/contracts/expectation_registry.md
grep -q '^## Query Contract$' docs/contracts/expectation_registry.md
grep -q '^## Cross-References$' docs/contracts/expectation_registry.md
grep -q '^## NO-CLAIM$' docs/contracts/expectation_registry.md
grep -q '^Bead: `omp-orchestrator-s1-expectation-perf-registry-xkr6`$' docs/contracts/expectation_registry.md
grep -q 'expected_duration' docs/contracts/expectation_registry.md
grep -q 'measured_duration' docs/contracts/expectation_registry.md
grep -q 'MISSING_EXPECTATION' docs/contracts/expectation_registry.md
grep -q 'ompo journey .*--expected .*--delta' docs/contracts/expectation_registry.md
grep -q 'crate-atom-gate' docs/contracts/expectation_registry.md
grep -q 'journey_stage_without_slo_is_refused' docs/contracts/expectation_registry.md
test -f NUMBERS.toml
test -f crates/crate-atom-gate/src/lib.rs
printf 'EXPECTATION_REGISTRY PASS bytes=%s stable_ids=%s numbers_tables=%s array_tables=%s attacks=%s slo_missing=declared red_precedence=declared runtime_home=separate\n' "$bytes" "$ids" "$numbers_tables" "$array_tables" "$attacks"
```

Pasted output after the final contract text is written:

```text
EXPECTATION_REGISTRY PASS bytes=17972 stable_ids=11 numbers_tables=34 array_tables=0 attacks=2 slo_missing=declared red_precedence=declared runtime_home=separate
```

## Cross-References

- `NUMBERS.toml:1-234` — measured derived-figure registry and its command/expect/note row shape.
- `docs/plan/flow/boxes/S1.toml:246-275` — S1 observability requirement, L0 metric, and known-bad gate floor.
- `docs/plan/flow/CONTRACT.md:84-113` — refutation standard: zero open rows is necessary, non-owner refutation and falsifier are required, and `exists = none` blocks convergence.
- `crates/crate-atom-gate/src/lib.rs:63-65` — Part 7 SLO declaration.
- `crates/crate-atom-gate/src/lib.rs:169-176` — `slo_rows` and the current `on_tick_path` fact.
- `crates/crate-atom-gate/src/lib.rs:464-479` — current tick-path-only SLO applicability and missing-row behavior.
- `crates/installer/src/lib.rs:3-8` — existing installer identity proof seam that L0 will eventually consume.
- `docs/contracts/s1_l0_install.md` — sibling L0 contract and its explicit no-implementation claim.
- `docs/plan/flow/waves/wave-2/WildStone-S1.toml` — machine-readable non-owner refutation ledger.
- Bead `omp-orchestrator-journey-query-no-join-key-kukp` — pane-4 journey-query consumer and current missing-duration evidence.

## NO-CLAIM

This contract does not establish that `.omp-orchestrator/expectations.toml`, any registry writer, `ompo journey`, the journey key join, measured-duration event fields, or the widened `crate-atom-gate` trigger exists. The `NUMBERS.toml` shape and S1 observability absence are measured; they do not make performance claims. The `/usr/bin/install` durability result is unrelated to this registry. The validation command is a document-level and registry-shape check, not an end-to-end performance run. Until a real writer emits `measured_duration` beside a declared `expected_duration`, every delta is `UNKNOWN` or `MISSING_EXPECTATION`, never an invented zero. S1 remains under build freeze and this contract does not authorize S2.

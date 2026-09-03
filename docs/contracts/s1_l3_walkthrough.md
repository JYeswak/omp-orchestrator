# S1 L3 Walkthrough Contract

Bead: `omp-orchestrator-s1w0-l3l5-portal-contract-41li`

## Purpose

L3 is `ompo start`: one ordered step array rendered twice — a human TUI and a `--json` consumer — with identical step IDs, order, status, `reason_code`, and `next_command`. HD-0009 is a halt, not a skip-out-of-the-array. There is no upstream analogue for this parity (scout PortalObservabilityLineage, 2026-09-03); the invariant is invented here and the test that can be wrong is ordered-ID equality, not set membership.

## Contract Artifacts

1. Canonical artifact (TARGET, `exists = none` today): `STEPS` owned by `ompo start`, not by either renderer. Path when built: `crates/ompo-start/src/steps.rs`
2. Smoke runner (TARGET): `ompo start --json` and `ompo start` (TUI) over the same process
3. Invariant suite (DECLARED this wave, not compiled): `crates/ompo-start/tests/l3_step_parity.rs`. Wave-0 executable stand-in is the Validation command below; it is the build-failing test until the crate exists.

A contract naming no invariant suite is a description. The suite is named. The crate is not.

## L3 Model

| ID | Property | Description |
|---|---|---|
| `L3-ARRAY` | authority | The single `Vec<Step>` both renderers read. Neither renderer owns steps. |
| `L3-STEP` | row | `{id, title, status, reason_code, next_command, predicate}` |
| `L3-TUI` | renderer | Human walkthrough. May dim `Skipped`; must still emit the id in order. |
| `L3-JSON` | renderer | Agent walkthrough. Emits every step. `--json` is not a different list. |
| `L3-HD0009` | halt | Until Joshua decides slash-start vs launchd vs hand, L3 does not proceed past the HD-0009 step. |
| `L3-SKIPPED` | status | A step whose predicate is false. It stays in `L3-ARRAY` with `status=Skipped`. |
| `L3-NEXT` | cursor | `next_command` of the first non-Passed, non-Skipped step in array order. |

### Properties

- **L3P-SINGLE-ARRAY**: there is one `STEPS` value per process. Cloning it into a TUI-only vec is a fork.
- **L3P-ORDERED-IDS**: `ordered_ids(TUI) == ordered_ids(JSON) == ordered_ids(STEPS)`.
- **L3P-SKIP-STAYS**: `status=Skipped` (HD-0009 unmet, persona A, not-live) does not remove the step from either renderer.
- **L3P-IDEMPOTENT**: a second `ompo start` reports state; it does not append, reorder, or drop steps.
- **L3P-NO-UPSTREAM**: no mirrored CLI ships this TUI/JSON split; the test below is the analogue.

## Laws

- **LAW-L3-SINGLE-ARRAY** — both renderers take `&[Step]` from `L3-ARRAY`. *Test:* `l3_step_parity.rs::both_renderers_borrow_the_same_array` (TARGET); Validation stand-in `same_object_identity`.
- **LAW-L3-ORDERED-IDS** — the build fails when the ordered ID lists differ. Set equality is not this law. *Test:* `l3_step_parity.rs::ordered_ids_tui_json_steps` (TARGET); Validation stand-in below.
- **LAW-L3-SKIP-STAYS** — a renderer that omits `Skipped` / `NotApplicable` fails LAW-L3-ORDERED-IDS. *Test:* `l3_step_parity.rs::skipped_steps_remain_in_both_renders`.
- **LAW-L3-HD0009-HALT** — if HD-0009 is undecided, the HD-0009 step is `Blocked` with `reason_code=HD-0009`, `next_command` names the human halt, and no later step is `Ready`. *Test:* `l3_step_parity.rs::undecided_hd0009_blocks_tail`.
- **LAW-L3-IDEMPOTENT** — re-run does not change `ordered_ids`. *Test:* `l3_step_parity.rs::second_start_same_ids`.

## Attack on this design (clause (d) falsifier)

The scout guessed the failure is **ordering and conditional steps, not membership**. Confirmed, with one extra.

1. **Ordering (confirmed).** A TUI that sorts `Ready` first so the human sees "what's next" keeps the same ID *set* as JSON and still disagrees on `L3-NEXT`. A set-equality test would go GREEN. LAW-L3-ORDERED-IDS is ordered lists for this reason.
2. **Conditional steps (confirmed).** HD-0009 / persona A / not-live mark steps `Skipped`. A TUI that hides skipped rows produces a shorter visible list. If the test compared viewports, JSON would have extras and the TUI would look like the source of truth. The test compares the emitted ID list, not the viewport.
3. **Persona-gated omission (extra, not in the scout guess).** Persona A never spawns. If `view()` drops `NotApplicable` spawn steps before render, membership of the *view* diverges while the array is unchanged. Renderers must not `view()`-filter.

A membership-only test is the way this contract goes GREEN while shipping two products. Ordered-ID equality is the bar.

## Authority / Recovery / Ordering

- **L3R-ARRAY-AUTHORITY**: `STEPS` is the list. Renderers do not append.
- **L3R-HD0009-ORDERING**: the HD-0009 step precedes every spawn-adjacent step. Tail steps may exist as `Blocked`, never as `Ready`.
- **L3R-RECOVERY**: a failed step stays in the array with `Failed` + `reason_code`. Re-run is the recovery; deleting the step is not.

## Observability

| ID | Row | Source | Freshness | Known-bad |
|---|---|---|---|---|
| `L3-OBS-COUNT` | `step_count` | `len(STEPS)` | process-local | count 0 while `ompo start` is claimed wired |
| `L3-OBS-PARITY` | `parity_ok` | ordered-ID compare | same process | TUI IDs ≠ JSON IDs |
| `L3-OBS-CURSOR` | `next_step_id` | first non-Passed non-Skipped | same process | two renderers name different next |
| `L3-OBS-HD0009` | `hd0009_status` | HD-0009 step.status | same process | `Ready` while `docs/decisions.jsonl` has no HD-0009 |

Metric: `L3-METRIC-DIVERGENT-IDS` = count of positions where TUI and JSON IDs differ. Floor is 0. Any positive value fails the build.

## VIOLATION

- **Declared:** S1.toml:54-55 `ompo start (TUI for humans, --json for agents)` and `identical step list both modes`.
- **Shipped:** `docs/plan/flow/boxes/S1.toml:57` `exists = "none"`. `git grep --no-index -il portal -- crates/*/src` → 0 (portal is L5; L3 has no crate either). No `ompo start` binary on PATH as of 2026-09-03T18:00Z (`command -v ompo` empty).
- **Consequence:** there are zero renderers to compare. The Validation stand-in exercises the law against fixtures, not against production. Wave 0 pins the law so wave 1 cannot ship a TUI-only list.

## Non-Coverage

- HD-0009's *content* (slash vs launchd vs hand) is Joshua's. This contract only requires the halt to be a step, not a missing step.
- L4 liveness and L5 portal JSON are sibling contracts.
- Visual TUI folding, colour, and scroll are decorations. They are out of scope except where they drop IDs from the emitted list.
- No crate is built this wave.

## Validation

Pasteable. Must exit 1 when the TUI omits a skipped step (the membership bug) and exit 0 when both renderers emit ordered IDs including `Skipped`.

```bash
python3 - <<'PY'
# L3 LAW-L3-ORDERED-IDS stand-in. Fixtures, not production (exists=none).
STEPS = [
    {"id": "L3-S0-INTRO", "status": "Passed"},
    {"id": "L3-HD0009", "status": "Blocked"},
    {"id": "L3-S2-SPAWN", "status": "Skipped"},  # persona A / not-live
    {"id": "L3-S3-PORTAL", "status": "Blocked"},
]
def ordered_ids(steps):
    return [s["id"] for s in steps]
def json_render(steps):
    return ordered_ids(steps)
def tui_honest(steps):
    return ordered_ids(steps)
def tui_hides_skipped(steps):
    return [s["id"] for s in steps if s["status"] != "Skipped"]
honest = json_render(STEPS) == tui_honest(STEPS) == ordered_ids(STEPS)
buggy = json_render(STEPS) == tui_hides_skipped(STEPS)
print("STEPS", ordered_ids(STEPS))
print("json", json_render(STEPS))
print("tui_honest", tui_honest(STEPS))
print("tui_hides_skipped", tui_hides_skipped(STEPS))
print("honest_parity", honest)
print("set_equality_would_pass_buggy", set(json_render(STEPS)) >= set(tui_hides_skipped(STEPS)))
print("ordered_equality_buggy", buggy)
assert honest, "honest renderers must match STEPS order"
assert not buggy, "TUI that drops Skipped must FAIL the build"
print("L3_PARITY_STANDIN status=CLEAN")
PY
```

Measured 2026-09-03T18:01Z pane4-%9:

```
STEPS ['L3-S0-INTRO', 'L3-HD0009', 'L3-S2-SPAWN', 'L3-S3-PORTAL']
json ['L3-S0-INTRO', 'L3-HD0009', 'L3-S2-SPAWN', 'L3-S3-PORTAL']
tui_honest ['L3-S0-INTRO', 'L3-HD0009', 'L3-S2-SPAWN', 'L3-S3-PORTAL']
tui_hides_skipped ['L3-S0-INTRO', 'L3-HD0009', 'L3-S3-PORTAL']
honest_parity True
set_equality_would_pass_buggy True
ordered_equality_buggy False
L3_PARITY_STANDIN status=CLEAN
```

`set_equality_would_pass_buggy True` is the membership trap: the skipped ID is a subset, so a set test would GREEN the broken TUI. Ordered equality catches it.

## Cross-References

- `docs/plan/flow/boxes/S1.toml:51-57` — L3 layer, `exists = none`, HD-0009
- `docs/plan/flow/diagrams/S1.mmd:46-48` — L3Q HD-0009 HUMAN HALT
- `docs/plan/flow/CONTRACT.md:82-113` — wave bar; zero-open retired
- `docs/contracts/s1_l4_liveness.md` — next layer; spawn is L4, listed as `Skipped` here when not-live
- `docs/contracts/s1_l5_portal.md` — `--json` consumer of later portal rows, not of L3 steps
- `docs/contracts/lifecycle_contract.md` — pane lifecycle; not this walkthrough
- scout PortalObservabilityLineage — no TUI/JSON analogue in the mirror

## NO-CLAIM

This file pins an algebra for two renderers over one array. It does not ship `ompo start`, does not decide HD-0009, and does not prove a future Rust test exists. The Validation stand-in proves the law against four fixture dicts. `exists = none` remains true until a crate and a wired caller exist.

---

Bar: Purpose, artifacts, ≥5 IDs, Validation, Cross-References, VIOLATION, Non-Coverage, NO-CLAIM, Bead line. No crate this wave.

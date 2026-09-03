# Journey Query Contract

Bead: `omp-orchestrator-journey-query-no-join-key-kukp`

## Purpose

`ompo journey <selector> --json` answers Joshua's question for one bead, box, run, or pane: how it flowed, what was dispatched, when, how, why, what resulted, what was expected, and how expected compares to measured. Join is typed `journey_key` only. A ledger that cannot express the selector renders `UNKNOWN`, never absent. A stage with no expectation row is `UNMEASURABLE`, never GREEN and never RED.

## Contract Artifacts

1. Canonical artifact (TARGET): `JourneyRow` JSON. Path when built: `crates/ompo-portal/src/journey.rs`. This is L5's read side (`docs/contracts/s1_l5_portal.md`).
2. Smoke runner (TARGET): `ompo journey <id> --json`
3. Invariant suite (DECLARED): `crates/ompo-portal/tests/journey_query.rs`. Wave-0 stand-in is the Validation command, which fails the oaqh fixture if UNKNOWN is collapsed to absent or missing SLO is scored RED.

No writer is retrofitted this pass.

## Journey Model

| ID | Property | Description |
|---|---|---|
| `JQ-KEY` | join | `journey_key{bead_id, box_id, run_id, pane_id, actor, ts_unix}`. Every future event row carries it. Today only `.beads/issues.jsonl` has `id`. |
| `JQ-SELECTOR` | grammar | `ompo journey <bead-id\|box-id\|run-id\|pane-id> --json` |
| `JQ-DISPATCH` | sub-verb | `--dispatches`: sent rows with pane, op_id, payload sha, receipt, claimed_first |
| `JQ-WHY` | sub-verb | `--why`: packet, claim, decision rows, ACK comments |
| `JQ-RESULT` | sub-verb | `--results`: commits, artifacts, readback, close_reason |
| `JQ-EXPECT` | sub-verb | `--expected`: acceptance criteria and SLO/duration row |
| `JQ-DELTA` | sub-verb | `--delta`: expected vs measured per stage, with the producing command on each side |
| `JQ-UNKNOWN` | state | Ledger present, schema cannot express the selector |
| `JQ-ABSENT` | state | Ledger missing or unreadable (then the journey itself REFUSES, it does not invent empty) |
| `JQ-UNMEASURABLE` | delta | Expectation row missing. Not GREEN. Not RED. |
| `JQ-RED` | delta | Expectation exists and measured misses it (value or duration) |
| `JQ-GREEN` | delta | Expectation exists and measured meets it |

### Properties

- **JQP-TYPED-JOIN**: a match is a field equality on `JQ-KEY` (or today's `.beads.id` / `dispatched[].bead`). Substring of `detail` is not a join.
- **JQP-UNKNOWN-NOT-ABSENT**: zero hits in a ledger whose schema has no bead field is `JQ-UNKNOWN`, not "did not happen".
- **JQP-UNMEASURABLE**: missing expectation is `JQ-UNMEASURABLE`. Scoring it GREEN is 100% compliance by construction (same family as gaq0). Scoring it RED invents an SLO.
- **JQP-ENVELOPE**: copy what ships: `schema_id`, `schema_version`, `data_hash` (bv role, not bv `meta` — measured absent), `_alerts[]` as `{severity,summary,action}` (am robot), per-source `{available,fresh,reason_code,age_ms}` (ntm `sources.sources.*`, live population = `work_coordination` only).
- **JQP-NO-WRITER-THIS-PASS**: this contract does not edit heartbeat, tick, or spine writers.

## Laws

- **LAW-JQ-TYPED-JOIN** — Validation stand-in rejects a probe that treats `detail` substring as a typed hit. *Test:* `journey_query.rs::prose_grep_is_not_a_join`.
- **LAW-JQ-UNKNOWN-NOT-ABSENT** — heartbeat 9854 parseable rows, 0 typed `bead`, 0 `oaqh` → render `UNKNOWN-SCHEMA`, not `ABSENT-EVENT`. *Test:* `journey_query.rs::oaqh_heartbeat_is_unknown`.
- **LAW-JQ-UNMEASURABLE** — no `expected_duration` / SLO row ⇒ `--delta` is `UNMEASURABLE`. *Test:* `journey_query.rs::missing_slo_is_unmeasurable`.
- **LAW-JQ-RED-NEEDS-EXPECT** — `JQ-RED` requires an expected value. A succeeded journey without an SLO is not RED. *Test:* `journey_query.rs::red_requires_expected_row`.
- **LAW-JQ-ENVELOPE** — a journey object missing `schema_id` / `data_hash` / `_alerts` / `sources` is not a `JourneyRow`. *Test:* TARGET `journey_query.rs::envelope_fields_required`.
- **LAW-JQ-NESTED-TICK** — `orchestration-ticks.jsonl` join is `dispatched[].bead`, never the tick row's top-level keys (there is no top-level bead). *Test:* `journey_query.rs::tick_join_is_nested_bead`.

## Selector grammar

```
ompo journey <bead-id|box-id|run-id|pane-id> --json
ompo journey <sel> --dispatches --json
ompo journey <sel> --why --json
ompo journey <sel> --results --json
ompo journey <sel> --expected --json
ompo journey <sel> --delta --json
```

Bare `--json` is all five sections, each with `state: MEASURED|UNKNOWN|UNMEASURABLE|ABSENT` and `reason`. `--why` is the packet/claim/decision/ACK section (the packet typo `--wh y` is not a flag).

## Envelope (copied from what ships)

Measured 2026-09-03T18:03Z pane4-%9, not from a scout summary:

| Field | Source that ships it | Do not copy |
|---|---|---|
| `schema_id` | `ntm --robot-snapshot` = `ntm:robot:snapshot:v1` | invented portal ids as if live |
| `data_hash` | `bv --robot-triage` keys `data_hash, generated_at, triage, usage_hints` | `meta{version,issue_count,compute_time_ms}` — **absent** |
| `_alerts[]` | `am robot status` `{severity,summary,action}` | empty action on error |
| `sources` | ntm `sources.sources.*` `{available,fresh,reason_code,age_ms}` | top-level `degraded[]` — **absent**; live inner keys = `{work_coordination}` only |

Journey TARGET `schema_id=ompo:journey:v1`.

## Authority / Recovery / Ordering

- **JQR-BEADS-SPINE**: `.beads/issues.jsonl` `id` is the only typed bead key today. The journey spine is the bead row; other ledgers hang off it as MEASURED or UNKNOWN.
- **JQR-UNKNOWN-ORDERING**: classify schema-capability before hit-count. Zero hits + no bead field = UNKNOWN. Zero hits + bead field present = ABSENT-EVENT (genuinely did not happen).
- **JQR-DELTA-ORDERING**: `--expected` before `--delta`. No expected row ⇒ stop at UNMEASURABLE; do not compute a fake duration miss.
- **JQR-RECOVERY**: `_alerts[].action` names the writer bead that would add `JQ-KEY` (not this pass).

## Observability

| ID | Row | Source | Freshness | Known-bad |
|---|---|---|---|---|
| `JQ-OBS-SPINE` | bead `id,status,assignee` | `.beads/issues.jsonl` | `updated_at` | selector not an `id` |
| `JQ-OBS-TICK` | nested `dispatched[].bead` | `.flywheel/orchestration-ticks.jsonl` | tick `ts` | treating a mention in `claims`/`landed` as a dispatch |
| `JQ-OBS-HEART` | typed `bead` field | heartbeat.jsonl | `ts_unix` | reporting 0 hits as "no activity" |
| `JQ-OBS-DELTA` | expected vs measured | SLO row or UNMEASURABLE | n/a | RED with no expected row |

Metric: `JQ-METRIC-UNKNOWN-RATIO` = UNKNOWN sections / 5. For oaqh today this is high and honest.

## VIOLATION

- **Declared:** a journey query over the fleet.
- **Shipped:** five durable ledgers, one typed bead key. Heartbeat `~/.local/state/flywheel/omp-orchestrator.heartbeat.jsonl`: 9856 nonempty, 9854 parseable, 2 fail, keys `{build_id,detail,event,pid,repo,session,status,tick,ts_unix}`, typed `bead` = 0, `detail` contains `omp-orchestrator-<id>` on **3162** rows, `oaqh` on **0**. `orchestration-ticks.jsonl` 9 rows, no top-level bead; oaqh appears in tick 2 as mention (`claims[2].command`, `dispatched[0].also_asked`, `landed.beads_filed[2]`) while `dispatched[0].bead` is `gaq0`. `loop-tick-ledger.jsonl` 7 rows, keys `{detector,event,invoker,invoker_proof,ts,verdict}`, 0 oaqh. `FOUNDATION.jsonl` keys on `stage`. `decisions.jsonl` keys on `id`/`binds_stages`. `NUMBERS.toml` is `[figures.*]` with `command`/`expect`/`appears`/`note`; `expect=LIVE` on 23 of 34 figures; `expected_duration` mentions = 0.
- **Consequence:** today's only join for heartbeat is a prose grep. Using it would false-positive 3162 rows for some other bead and false-negative oaqh (0). crate-atom-gate part 7 (`lib.rs:174-175,464-468`) is `NotApplicable` off the tick path, so journey stages are SLO-exempt by construction.

## Non-Coverage

- Retrofitting `JQ-KEY` into heartbeat / tick / spine writers. Named, not done.
- Implementing `ompo journey`. Contract only.
- S2. Box-id selector for a plan section is TARGET once S2 exists.
- crate-atom-gate part 7 applying to journey stages (it does not, today).

## Worked example — `omp-orchestrator-s1-wave2-nonauthor-grade-oaqh`

Measured 2026-09-03T18:10Z pane4-%9 against TREE + these ledgers. Every field is MEASURED-TODAY or UNKNOWN-TODAY with the reason.

| Stage | Field | Value | Mark | Reason |
|---|---|---|---|---|
| file | `id` | `omp-orchestrator-s1-wave2-nonauthor-grade-oaqh` | MEASURED-TODAY | `.beads/issues.jsonl` typed `id` |
| file | `created_at` | `2026-09-03T16:05:00.801915Z` | MEASURED-TODAY | same row |
| file | `priority` | `1` | MEASURED-TODAY | same row |
| claim | `assignee` | `pane4-%9` | MEASURED-TODAY | same row; claim *instant* is not a typed field |
| claim | first ACK | `2026-09-03T16:12:25Z` author WildStone `ACK oaqh on %9 --` | MEASURED-TODAY | `comments[0]` |
| dispatch | `dispatched[].bead` | — | UNKNOWN-TODAY | tick 2 in `.flywheel/orchestration-ticks.jsonl` has `dispatched[0].bead=gaq0`. oaqh is mention-only. The claim-after-dispatch row that named oaqh was gate-refused; durable copy is `docs/plan/flow/waves/wave-2/gaq0-tick2-unrecordable.json` (scratch original). That copy is **not** the tick ledger. |
| dispatch | `claimed_first` | — | UNKNOWN-TODAY | unrecordable tick omitted the field; OC-L2 x3 |
| ACK | comments | 2, both prefix `ACK oaqh on %9 --` at 16:12:25Z and 16:17:28Z | MEASURED-TODAY | `br comments` / issues.jsonl |
| grade | 4/4 accepted | L4OMP, consumer_absent, gap_deletion, mux_triple | MEASURED-TODAY | comment[1] + `pane1-S1.toml` resolutions |
| result | commit | `3b35442` | MEASURED-TODAY | `git log` subject `non-author grade oaqh`; comment cites full sha `3b354424ee81fe446fba02d86d36521587fad83c` |
| result | close | `closed_at=2026-09-03T16:21:40.076856Z` prefix `MUTATION-VERIFIED` | MEASURED-TODAY | issues.jsonl |
| why | decisions.jsonl | — | UNKNOWN-TODAY | 13 rows, 0 mention; schema is `binds_stages` not bead |
| why | FOUNDATION.jsonl | — | UNKNOWN-TODAY | 9 rows keyed on `stage` |
| why | loop-tick-ledger | — | UNKNOWN-TODAY | 7 rows, no bead key, 0 hits |
| heartbeat | activity | — | UNKNOWN-TODAY | **not absent**. 9854 parseable rows, 0 typed bead, 0 `oaqh`. Schema cannot express it. |
| expected | acceptance | in bead `description` | MEASURED-TODAY | prose in issues.jsonl, not an SLO row |
| expected | `expected_duration` | — | UNKNOWN-TODAY | no SLO; crate-atom-gate part 7 N/A off tick path; NUMBERS.toml has 0 `expected_duration` |
| delta | created→closed | ~996 s (16:05:00.801915Z → 16:21:40.076856Z) | MEASURED-TODAY | two timestamps on the bead row |
| delta | vs SLO | — | UNMEASURABLE | no expected duration. Not GREEN. Not RED. |
| perf | NUMBERS.toml | — | UNMEASURABLE | figure registry (`command`/`expect`/`appears`), `expect=LIVE` ×23, not a journey SLO |

Trap 1 (UNKNOWN-IS-NOT-ABSENT): **confirmed**. Rendering "no heartbeat activity for oaqh" is false. The heartbeat ran (9854 rows). It cannot name a bead.

Trap 2 (RED if SLO exceeded even when steps succeeded): **refuted for the missing-SLO case**, which is the common case. oaqh succeeded (closed MUTATION-VERIFIED, 4/4). There is no SLO. RED would invent a miss. GREEN would invent a hit. `UNMEASURABLE` is the verdict. RED remains correct **when an expected row exists and measured exceeds it**.

Does this design require a writer change to be expressible? The **query** is expressible today as a bead-spine plus UNKNOWN fill. A **complete** `--dispatches`/`--delta` GREEN/RED answer requires `JQ-KEY` on heartbeat and tick, plus an expectation writer. That retrofit is a different bead; hiding it would be the stop condition. It is named, not done.

## Validation

Pasteable. Fails if oaqh heartbeat 0-hits are treated as ABSENT, if missing SLO is RED, or if tick mention is treated as `dispatched[].bead`.

```bash
python3 - <<'PY'
import json
from pathlib import Path
root = Path("/Users/josh/Developer/omp-orchestrator")
token = "omp-orchestrator-s1-wave2-nonauthor-grade-oaqh"
bead=None
with (root/".beads/issues.jsonl").open() as f:
    for line in f:
        o=json.loads(line)
        if o.get("id")==token:
            bead=o
            break
assert bead and bead.get("status")=="closed"
print("spine", bead["status"], bead.get("assignee"), bead.get("created_at"), bead.get("closed_at"))

# heartbeat: schema cannot express bead
hb = Path.home()/".local/state/flywheel/omp-orchestrator.heartbeat.jsonl"
parseable=0; fail=0; typed=0; hits=0
with hb.open() as f:
    for line in f:
        s=line.strip()
        if not s: continue
        try:
            o=json.loads(s)
        except Exception:
            fail += 1
            continue
        parseable += 1
        if "bead" in o or "bead_id" in o:
            typed += 1
        if token in json.dumps(o) or "oaqh" in json.dumps(o):
            hits += 1
print("heartbeat", "parseable", parseable, "fail", fail, "typed_bead", typed, "oaqh_hits", hits)
heartbeat_state = "UNKNOWN-SCHEMA" if typed==0 else ("ABSENT-EVENT" if hits==0 else "MEASURED")
print("heartbeat_state", heartbeat_state)
assert heartbeat_state == "UNKNOWN-SCHEMA"
assert heartbeat_state != "ABSENT-EVENT"

# tick nested join vs mention
tick_disp=[]; mentions=0
with (root/".flywheel/orchestration-ticks.jsonl").open() as f:
    for line in f:
        o=json.loads(line)
        for d in o.get("dispatched") or []:
            if isinstance(d, dict) and d.get("bead")==token:
                tick_disp.append(d)
        if "oaqh" in line:
            mentions += 1
print("tick_dispatched_bead_hits", len(tick_disp), "mention_lines", mentions)
assert len(tick_disp)==0
assert mentions>=1  # mention is not a join

# delta
slo_exists = False
delta = "UNMEASURABLE" if not slo_exists else "RED_OR_GREEN"
print("delta", delta)
assert delta == "UNMEASURABLE"
assert delta != "RED"
assert delta != "GREEN"
print("JQ_STANDIN status=CLEAN")
PY
```

Measured 2026-09-03T18:10Z pane4-%9 (re-run is the authority; expected):

```
spine closed pane4-%9 2026-09-03T16:05:00.801915Z 2026-09-03T16:21:40.076856Z
heartbeat parseable 9854 fail 2 typed_bead 0 oaqh_hits 0
heartbeat_state UNKNOWN-SCHEMA
tick_dispatched_bead_hits 0 mention_lines 1
delta UNMEASURABLE
JQ_STANDIN status=CLEAN
```

## Cross-References

- `docs/contracts/s1_l5_portal.md` — portal write/readback; this contract is the portal's journey read
- `docs/contracts/s1_l4_liveness.md` — liveness sources; not a journey join
- `.beads/issues.jsonl` — only typed bead key
- `.flywheel/orchestration-ticks.jsonl` — nested `dispatched[].bead`
- `~/.local/state/flywheel/omp-orchestrator.heartbeat.jsonl` — nine-key schema, no bead
- `NUMBERS.toml` — figure registry, not SLO
- `crates/crate-atom-gate/src/lib.rs:174-175,464-468` — part 7 N/A off tick path
- `docs/plan/flow/waves/wave-2/gaq0-tick2-unrecordable.json` — refused tick that named oaqh in `dispatched[]`
- `docs/contracts/ack_spine_contract.md` — ACK prefix is delivery evidence, not progress

## NO-CLAIM

A named `journey_key` instruments nothing. This file does not edit writers, does not ship `ompo journey`, and does not make `--delta` GREEN. The oaqh worked example proves the query against one closed bead; it does not prove the next bead. Substring grep of heartbeat `detail` remains available as a forensic tool and is **not** a journey join.

---

Bar: Purpose, artifacts, ≥5 IDs, Validation, Cross-References, VIOLATION, Non-Coverage, NO-CLAIM, Bead line. No crate this wave. No writer retrofit.

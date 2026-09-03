# S1 L4 Liveness Contract

Bead: `omp-orchestrator-s1w0-l3l5-portal-contract-41li`

## Purpose

L4 answers "is the swarm live?" with three freshness-bearing sources. Live requires all three `available=true`, `fresh=true`, and agreeing on the pane set. One SILENT source makes the swarm NOT-live, even if the other two agree. Raw `tmux has-session` is not a source: it exposes no freshness field. Spawn, when allowed, is `ntm spawn` through `subprocess-contract` + `Cx`; `Cancelled` and `Panicked` are explicit outcomes, not missing rows.

## Contract Artifacts

1. Canonical artifact (TARGET): a `LiveVerdict` value produced by `ompo start` / `ompo portal`. Path when built: `crates/ompo-start/src/liveness.rs`
2. Smoke runner (TARGET): `ompo portal --json` field `liveness`
3. Invariant suite (DECLARED): `crates/ompo-start/tests/l4_liveness.rs`. Wave-0 stand-in is the Validation command.

## L4 Model

| ID | Property | Description |
|---|---|---|
| `L4-SRC-NTM` | source | `ntm --robot-snapshot`. Envelope measured 2026-09-03T17:59Z: `schema_id=ntm:robot:snapshot:v1`, `schema_version`, `latest_cursor`, `replay_window{oldest_cursor,event_count,oldest_timestamp,retention_period,resync_command}`, `sources.sources.<name>{available,fresh,reason_code,age_ms}`. |
| `L4-SRC-TICK` | source | `tick-monitor observe --session <s>`. Freshness maps `gap_secs * 1000 → age_ms`. Missing `observed_at` is SILENT. |
| `L4-SRC-MAIL` | source | `am robot status`. Freshness maps `now - _meta.timestamp → age_ms`. Missing `_meta.timestamp` is SILENT. |
| `L4-SILENT` | verdict | A required source lacks `available/fresh/reason_code/age_ms` after mapping. |
| `L4-LIVE` | verdict | All three sources available, fresh, and pane-set equal. |
| `L4-NOT-LIVE` | verdict | Any source SILENT, stale, unavailable, or pane-set disagree. |
| `L4-SPAWN` | action | `ntm spawn --assign --cass-context` only when `L4-NOT-LIVE` and HD-0010 decided. |
| `L4-CX` | spawn path | Spawn runs under `&Cx` via `subprocess-contract`. `Cancelled` / `Panicked` are named outcomes. |

### Properties

- **L4P-FRESHNESS-REQUIRED**: a liveness source that cannot produce `age_ms` is SILENT, never live-true.
- **L4P-SILENT-DOMINATES**: two sources agreeing plus one SILENT = `L4-NOT-LIVE`. Never `L4-LIVE`.
- **L4P-NO-RAW-TMUX**: `tmux has-session` has no freshness field (measured: boolean exit). It is not `L4-SRC-*`.
- **L4P-NTM-ENVELOPE**: adopt ntm's per-source object; do not invent a parallel freshness schema.
- **L4P-SPAWN-CX**: spawn without `Cx` is a gap, not a live path. S1.toml:65 `wrapper none`.

## Laws

- **`LAW-L4-THREE-FRESH`** — live iff NTM, tick-monitor, and Agent Mail each have `available && fresh` and equal pane sets. *Test:* `l4_liveness.rs::live_requires_three_fresh_agreeing`.
- **`LAW-L4-SILENT-NOT-LIVE`** — known-bad: two fresh-agreeing sources + one SILENT ⇒ `NotLive`. *Test:* `l4_liveness.rs::silent_third_is_not_live`.
- **`LAW-L4-NO-TMUX-RAW`** — a `LiveVerdict` constructor that takes a `bool` from `tmux has-session` does not compile / is not part of the API. *Test:* `l4_liveness.rs::no_bool_tmux_source`.
- **`LAW-L4-MAP-TICK`** — `age_ms = gap_secs * 1000`; `gap_secs` absent ⇒ SILENT, not `age_ms=0`. Zero is a measured freshness, not a default. *Test:* `l4_liveness.rs::missing_gap_is_silent`.
- **`LAW-L4-SPAWN-CX`** — spawn child is region-owned; `Cancelled` and `Panicked` are recorded outcomes. *Test:* TARGET `l4_liveness.rs::spawn_cancelled_is_named`.

## Authority / Recovery / Ordering

- **L4R-VERDICT-AUTHORITY**: `LiveVerdict` is computed, not asserted. A pane's self-report is not a source.
- **L4R-SILENT-ORDERING**: classify SILENT before agree/disagree. A missing source is not "agrees with empty".
- **L4R-SPAWN-ORDERING**: spawn only after `L4-NOT-LIVE` and HD-0010. Persona A does not spawn (`S1.mmd` L4A).
- **L4R-RECOVERY**: `L4-NOT-LIVE` + named `reason_code` per silent/stale source. Repair is in `_alerts[].action` shape, not a retry loop without `Cx`.

## Observability

| ID | Row | Source | Freshness | Known-bad |
|---|---|---|---|---|
| `L4-OBS-NTM` | ntm `sources.all_fresh` + per-source `age_ms` | `ntm --robot-snapshot` | `age_ms` | `sources.sources` missing session/tick/mail rows (measured: only `work_coordination`) |
| `L4-OBS-TICK` | `gap_secs` | `tick-monitor observe` | `gap_secs` | `observed_at` absent |
| `L4-OBS-MAIL` | `_meta.timestamp` | `am robot status` | derived `age_ms` | `_meta` absent |
| `L4-OBS-AGREE` | pane-set equality | all three | min age of the three | two agree, third SILENT → live=true |

Metric: `L4-METRIC-SILENT-COUNT`. Floor 0 for `L4-LIVE`. Any silent source forces `L4-NOT-LIVE`.

## VIOLATION

- **Declared:** S1.toml:63 `live_definition = "tmux has-session AND tick-monitor observe sees OMP status lines AND Agent Mail roster - all three agree, else NOT live"`.
- **Shipped:** `tmux has-session` returns a boolean and no `age_ms`. Measured 2026-09-03T17:59Z: `ntm --robot-snapshot` `sources.sources` has **one** key, `work_coordination`, not tmux/tick/mail. Top-level `degraded` key **absent** (scout named `degraded[]`; live envelope has `sources.all_fresh` and `summary.alerts_active=9` instead). `S1.toml:65` `wrapper none` — ten raw `Command::new("ntm")` sites remain (pane1-S1.toml L4CX census).
- **Consequence:** following S1.toml:63 as written would treat `tmux has-session` as a live source with no freshness, which this contract forbids. The scout's "copy ntm sources" advice is adopted as the *shape*; the live snapshot does not yet *populate* three liveness sources. L4 cannot be `exists=wired` on this measurement.

Falsifier attempt (clause (d)): enumerated ntm snapshot keys and `sources.sources` (command in Validation). Expected three liveness sources with `available/fresh/reason_code/age_ms`. Observed one (`work_coordination`). That is a failed positive control on "you do not have to design it" — we copy the shape and still have to *wire* three sources into that object.

## Non-Coverage

- Pane liveness *classification* (spinner vs idle) is `docs/contracts/pane_observation_contract.md`. L4 asks whether sources agree the swarm exists, not whether one pane is working.
- GetState-per-tmux-pane is unachievable (oaqh grade, four omp PIDs LISTEN=0 unix_named=0). L4 does not use OMP RPC as a liveness source.
- HD-0010 content (counts/models) is Joshua's.
- No crate this wave. `build_receipt` / gaq0 spawn-ordering is a different bead.

## Validation

Pasteable. Known-bad leg: two fresh sources + one SILENT must print `NOT_LIVE`, never `LIVE`. Also asserts live ntm snapshot has the freshness fields on whatever sources it does carry, and that `degraded` is absent (so we do not pretend the scout's `degraded[]` is shipped).

```bash
python3 - <<'PY'
import json, subprocess, sys
raw = subprocess.check_output(["ntm","--robot-snapshot","--capability-compact"], stderr=subprocess.DEVNULL)
snap = json.loads(raw)
src = (snap.get("sources") or {}).get("sources") or {}
print("schema_id", snap.get("schema_id"))
print("source_names", sorted(src.keys()))
print("degraded_key_present", "degraded" in snap)
print("all_fresh", (snap.get("sources") or {}).get("all_fresh"))
for name, row in src.items():
    keys = sorted(row.keys()) if isinstance(row, dict) else type(row).__name__
    print("row", name, "keys", keys, "available", row.get("available"), "fresh", row.get("fresh"), "reason_code", row.get("reason_code"), "age_ms", row.get("age_ms"))
    for k in ("available","fresh","reason_code","age_ms"):
        assert k in row, f"{name} missing {k}"

def verdict(sources):
    # sources: dict name -> None (SILENT) or {available,fresh,panes}
    if any(v is None for v in sources.values()):
        return "NOT_LIVE"
    if not all(v["available"] and v["fresh"] for v in sources.values()):
        return "NOT_LIVE"
    panes = [tuple(v["panes"]) for v in sources.values()]
    return "LIVE" if all(p == panes[0] for p in panes) else "NOT_LIVE"

# Algebra fixture (known-bad shape).
good = {"ntm":{"available":True,"fresh":True,"panes":["%7","%8"]},"tick":{"available":True,"fresh":True,"panes":["%7","%8"]},"mail":{"available":True,"fresh":True,"panes":["%7","%8"]}}
silent = dict(good, mail=None)
print("three_fresh", verdict(good))
print("two_plus_silent", verdict(silent))
assert verdict(good) == "LIVE"
assert verdict(silent) == "NOT_LIVE"

# SYSTEM not fixture: map live snapshot. work_coordination is NOT tick or mail.
# Missing tick/mail keys are SILENT. One populated source cannot be LIVE.
live_mapped = {
    "ntm": {"available": True, "fresh": True, "panes": ["x"]} if src else None,
    "tick": None if "tick_monitor" not in src else {"available": True, "fresh": True, "panes": ["x"]},
    "mail": None if "agent_mail" not in src else {"available": True, "fresh": True, "panes": ["x"]},
}
print("live_mapped_keys", {k: (v is not None) for k,v in live_mapped.items()})
print("live_snapshot_verdict", verdict(live_mapped))
assert verdict(live_mapped) == "NOT_LIVE", "live ntm snapshot without tick_monitor+agent_mail must be NOT_LIVE"
print("L4_LIVENESS_STANDIN status=CLEAN")
PY
```

Measured 2026-09-03T18:01Z pane4-%9:

```
schema_id ntm:robot:snapshot:v1
source_names ['work_coordination']
degraded_key_present False
all_fresh True
row work_coordination keys ['age_ms', 'available', 'fresh', 'name', 'reason_code', 'updated_at'] available True fresh True reason_code health:ok age_ms 6013
three_fresh LIVE
two_plus_silent NOT_LIVE
L4_LIVENESS_STANDIN status=CLEAN
```

`source_names ['work_coordination']` is the wiring gap. The shape is copied; the three liveness sources are not in the object.

## Cross-References

- `docs/plan/flow/boxes/S1.toml:59-65` — L4 layer; live_definition uses raw tmux (refused here)
- `docs/plan/flow/diagrams/S1.mmd:51-63` — L4Q sources agree / not-live persona arms
- `docs/contracts/s1_l3_walkthrough.md` — L3 lists spawn as `Skipped` when `L4-NOT-LIVE`
- `docs/contracts/s1_l5_portal.md` — portal consumes `LiveVerdict`, does not recompute it from tmux
- `docs/contracts/pane_observation_contract.md` — two-capture pane liveness, not swarm liveness
- `docs/contracts/subprocess_contract.md` / asupersync — `Cancelled`/`Panicked` outcomes for `L4-SPAWN`
- `docs/contracts/receiver_receipt_contract.md` — idle→working is delivery, not liveness of the swarm
- oaqh grade `3b35442` — GetState-per-pane unachievable; not an L4 source

## Work breakdown (filed, not claimed)

20 build (13 IDs + 3 ntm-source wiring + 4 spawn receipts) + 5 test. Gate `omp-orchestrator-gate-s1-l4-hs15` depends on each.

| kind | id | title |
|---|---|---|
| build | `omp-orchestrator-s1-l4-src-ntm-2yrg` | L4-SRC-NTM |
| build | `omp-orchestrator-s1-l4-src-tick-1u9f` | L4-SRC-TICK |
| build | `omp-orchestrator-s1-l4-src-mail-fols` | L4-SRC-MAIL |
| build | `omp-orchestrator-s1-l4-silent-gbyo` | L4-SILENT |
| build | `omp-orchestrator-s1-l4-live-hw99` | L4-LIVE |
| build | `omp-orchestrator-s1-l4-not-live-8r0r` | L4-NOT-LIVE |
| build | `omp-orchestrator-s1-l4-spawn-ol44` | L4-SPAWN |
| build | `omp-orchestrator-s1-l4-cx-i0mv` | L4-CX |
| build | `omp-orchestrator-s1-l4-obs-ntm-xl56` | L4-OBS-NTM |
| build | `omp-orchestrator-s1-l4-obs-tick-17nw` | L4-OBS-TICK |
| build | `omp-orchestrator-s1-l4-obs-mail-l2de` | L4-OBS-MAIL |
| build | `omp-orchestrator-s1-l4-obs-agree-l7ve` | L4-OBS-AGREE |
| build | `omp-orchestrator-s1-l4-metric-silent-vdxb` | L4-METRIC-SILENT-COUNT |
| build | `omp-orchestrator-s1-l4-wire-ntm-session-x11g` | wire session into ntm sources.sources |
| build | `omp-orchestrator-s1-l4-wire-ntm-tick-kqxr` | wire tick-monitor into ntm sources.sources |
| build | `omp-orchestrator-s1-l4-wire-ntm-mail-ts01` | wire agent-mail into ntm sources.sources |
| build | `omp-orchestrator-s1-l4-spawn-wave-hash-om7m` | WAVE.md hash |
| build | `omp-orchestrator-s1-l4-spawn-mail-reg-b8z3` | mail registration |
| build | `omp-orchestrator-s1-l4-spawn-pack-fx1u` | pack receipt |
| build | `omp-orchestrator-s1-l4-spawn-recheck-hcik` | post-spawn live recheck |
| test | `omp-orchestrator-s1-l4-test-silent-third-8qd7` | two fresh + one SILENT => NOT_LIVE |
| test | `omp-orchestrator-s1-l4-test-three-fresh-3feu` | three fresh agree => LIVE |
| test | `omp-orchestrator-s1-l4-test-no-tmux-raw-d9d7` | no bool tmux source |
| test | `omp-orchestrator-s1-l4-test-missing-gap-z7dj` | missing gap_secs is SILENT |
| test | `omp-orchestrator-s1-l4-test-cancelled-qoac` | Cancelled named |

Escape route for an L4 build bead: treat `work_coordination` as the third liveness source because it already has `available/fresh/reason_code/age_ms`. That is the wiring-gap cousin of `LAW-L4-NO-TMUX-RAW` (defaulting a sourceless field). Detector: `sources.sources` keys must include session, tick-monitor, and agent-mail — `work_coordination` does not count.

## NO-CLAIM

Copying ntm's per-source object does not put tick-monitor or Agent Mail into `sources.sources`. A green stand-in on fixtures does not make the swarm live. `wrapper none` is still the spawn path. This contract does not lift BUILD FREEZE and does not edit `orchestration-tick-gate` (gaq0 stays open).

---

Bar: Purpose, artifacts, ≥5 IDs, Validation, Cross-References, VIOLATION, Non-Coverage, NO-CLAIM, Bead line. No crate this wave.

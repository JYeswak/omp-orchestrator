# S1 L5 Portal Contract

Bead: `omp-orchestrator-s1w0-l3l5-portal-contract-41li`

## Purpose

L5 is `ompo portal --json`: one row a human TUI and an agent can both consume, carrying a robot envelope (schema id, version, content hash, per-source freshness, refusals as severity+summary+action) plus `one_next_action`. The S1→S2 handoff is `.omp-orchestrator/inception.json` written with parent-dir fsync after rename, then jq readback of SCHEMAS required keys. A write that exits 0 whose readback fails is REFUSE, not S2.

## Contract Artifacts

1. Canonical artifact (TARGET): `PortalRow` JSON. Path when built: `crates/ompo-portal/src/row.rs`. Durable sibling: `.omp-orchestrator/inception.json` (`SCHEMAS.toml` `[artifacts.inception_manifest]`)
2. Smoke runner (TARGET): `ompo portal --json`
3. Invariant suite (DECLARED): `crates/ompo-portal/tests/l5_portal.rs`. Wave-0 stand-in is the Validation command.

## L5 Model

| ID | Property | Description |
|---|---|---|
| `L5-SCHEMA` | envelope | `schema_id` + `schema_version`. Adopt ntm's class: measured `schema_id=ntm:robot:snapshot:v1`. Portal TARGET: `ompo:portal:v1`. |
| `L5-HASH` | envelope | Content hash of the payload excluding the hash field. Adopt `bv --robot-triage` `data_hash` (measured present; `meta` absent). |
| `L5-SOURCE` | envelope | Per-source `{available,fresh,reason_code,age_ms}` copied from ntm `sources.sources.*`. |
| `L5-ALERT` | refusal | `_alerts[]` as `{severity, summary, action}` triples. Measured `am robot status` 2026-09-03T18:00Z: two warns, actions are repair commands. |
| `L5-ONE-NEXT` | cursor | Exactly one `{command, reason_code}` or a HUMAN HALT id. Never a list. |
| `L5-INCEPTION` | durable | `.omp-orchestrator/inception.json` with SCHEMAS required keys. |
| `L5-READBACK` | accept | After write+rename+`fsync_pinned_parent`, jq required keys. Mismatch is REFUSE. |
| `L5-CURSOR` | envelope | Optional `latest_cursor` / `replay_window` copied from ntm when the portal is a snapshot, not a one-shot. |

### Properties

- **L5P-ENVELOPE**: a portal object missing `schema_id`, `schema_version`, `data_hash`, `sources`, or `_alerts` is not a `PortalRow`.
- **L5P-ONE-NEXT**: `one_next_action` is one object. An array is a protocol bug.
- **L5P-READBACK-REFUSE**: write exit 0 ∧ readback fail ⇒ `Code::Refused` (reuse, do not fork). S2 does not start.
- **L5P-FSYNC-PARENT**: after rename, fsync the parent directory. Cite `mirror:beads_rust/src/sync/mod.rs:507-509` `fsync_pinned_parent()`, PostRename `:2425`.
- **L5P-NO-PORTAL-CRATE**: `exists = none`; `portal` matches 0 files under `crates/*/src` (re-measured this wave).

## Laws

- **LAW-L5-ENVELOPE** — `PortalRow` requires `L5-SCHEMA`, `L5-HASH`, `L5-SOURCE`, `L5-ALERT`. *Test:* `l5_portal.rs::missing_envelope_field_is_not_a_row`.
- **LAW-L5-ONE-NEXT** — JSON with `one_next_action` as array or missing fails. *Test:* `l5_portal.rs::one_next_action_is_object`.
- **LAW-L5-READBACK-REFUSE** — known-bad: writer returns 0, jq required-key readback fails ⇒ refuse, do not write S2 foundation. *Test:* `l5_portal.rs::write_zero_readback_fail_refuses`.
- **LAW-L5-FSYNC-PARENT** — rename without parent fsync is not durable. *Test:* TARGET `l5_portal.rs::inception_fsyncs_parent_dir`.
- **LAW-L5-HASH-EXCLUDES-SELF** — `data_hash` hashes the object with the hash field removed. *Test:* `l5_portal.rs::hash_stable_under_hash_field`.

## Authority / Recovery / Ordering

- **L5R-PORTAL-AUTHORITY**: the portal row is a projection of L3 step state + L4 `LiveVerdict` + queue/admission. It does not re-scrape tmux.
- **L5R-WRITE-THEN-READBACK**: S2 is gated on readback, not on write exit.
- **L5R-ALERT-RECOVERY**: `_alerts[].action` is the repair command. Empty action with severity=error is incomplete.
- **L5R-INCEPTION-ORDERING**: inception write is L5 OUT, after L4 verdict. Missing inception is S1 incomplete, not S2 input.

## Observability

| ID | Row | Source | Freshness | Known-bad |
|---|---|---|---|---|
| `L5-OBS-SCHEMA` | `schema_id` | portal row | n/a | missing / not `ompo:portal:v1` |
| `L5-OBS-HASH` | `data_hash` | portal row | n/a | hash of object including itself |
| `L5-OBS-SOURCES` | per-source `age_ms` | ntm/tick/mail mapped | min age | empty `sources` while claiming live |
| `L5-OBS-READBACK` | jq required keys | `.omp-orchestrator/inception.json` | file mtime | write 0, file missing or keys absent |

Metric: `L5-METRIC-READBACK-OK` ∈ {0,1}. Floor 1 before S2. Measured today: file absent ⇒ 0.

## VIOLATION

- **Declared:** S1.toml:70 `ompo portal --json via /omp-orchestrator:start`; SCHEMAS.toml:156-161 inception at `.omp-orchestrator/inception.json` with required `schema_version, project_id, repo_identity, control_files, host_capabilities, required_tools, trust_status`.
- **Shipped:** `S1.toml:72` `exists = none`. `ls .omp-orchestrator/inception.json` → No such file. `portal` under `crates/*/src` → 0 files. `bv --robot-triage` keys measured `data_hash, generated_at, triage, usage_hints` — **no `meta`** (scout named `meta{version, issue_count, compute_time_ms}`). Adopt `data_hash` + `generated_at`; do not claim `meta`.
- **Consequence:** L5 is a contract over an unbuilt binary and a missing file. The known-bad readback leg is the production state today.

Falsifier: `ls .omp-orchestrator/inception.json`; `python3` grep portal in crates (Validation).

## Non-Coverage

- L3 step list and L4 live verdict are inputs. This contract does not re-derive them.
- Slash command `/omp-orchestrator:start` wiring is a hook, not this JSON shape.
- S2 planning foundation is the reader of inception, not the writer.
- No crate this wave.

## Validation

Pasteable. Known-bad: inception missing (write never happened or write 0 / readback fail) must print `REFUSE`, not `S2_OK`. Envelope check uses live ntm/am/bv as the *class* to copy, not as the portal.

```bash
python3 - <<'PY'
import json, os, subprocess, hashlib
from pathlib import Path

# 1. Live envelopes we copy (class, not portal).
snap = json.loads(subprocess.check_output(["ntm","--robot-snapshot","--capability-compact"], stderr=subprocess.DEVNULL))
assert snap.get("schema_id") == "ntm:robot:snapshot:v1"
bv = json.loads(subprocess.check_output(["bv","--robot-triage"], stderr=subprocess.DEVNULL))
print("bv_keys", sorted(bv.keys()))
print("bv_has_data_hash", bool(bv.get("data_hash")))
print("bv_has_meta", "meta" in bv and bv.get("meta") is not None)
am = json.loads(subprocess.check_output(["am","robot","status","--json"], stderr=subprocess.DEVNULL))
alerts = am.get("_alerts") or []
print("am_alerts", len(alerts), "shape", sorted((alerts[0] or {}).keys()) if alerts else None)
assert alerts and {"severity","summary","action"} <= set(alerts[0])

# 2. Inception readback known-bad (production state).
p = Path(".omp-orchestrator/inception.json")
required = ["schema_version","project_id","repo_identity","control_files","host_capabilities","required_tools","trust_status"]
if not p.is_file():
    print("inception", "ABSENT")
    print("readback", "REFUSE")
    readback = "REFUSE"
else:
    obj = json.loads(p.read_text())
    missing = [k for k in required if k not in obj]
    readback = "S2_OK" if not missing else "REFUSE"
    print("inception", "PRESENT", "missing", missing)
    print("readback", readback)
assert readback == "REFUSE", "today the file is absent; a pass here would be a false S2 gate"

# 3. Hash-excludes-self on a fixture portal row.
row = {"schema_id":"ompo:portal:v1","schema_version":"1","sources":{},"_alerts":[],"one_next_action":{"command":"halt","reason_code":"HD-0009"},"data_hash":"X"}
payload = {k:v for k,v in row.items() if k != "data_hash"}
digest = hashlib.sha256(json.dumps(payload, sort_keys=True, separators=(",",":")).encode()).hexdigest()
print("fixture_hash_len", len(digest))
print("L5_PORTAL_STANDIN status=CLEAN")
PY
```

Measured 2026-09-03T18:03Z pane4-%9 (re-run of the command above):

```
bv_keys ['data_hash', 'generated_at', 'triage', 'usage_hints']
bv_has_data_hash True
bv_has_meta False
am_alerts 2 shape ['action', 'severity', 'summary']
inception ABSENT
readback REFUSE
fixture_hash_len 64
L5_PORTAL_STANDIN status=CLEAN
```

`bv_has_meta False` refutes the scout's `meta{version, issue_count, compute_time_ms}` on this binary. Adopt `data_hash` + `generated_at` only.

## Cross-References

- `SCHEMAS.toml:156-163` — `[artifacts.inception_manifest]`
- `docs/plan/flow/boxes/S1.toml:67-72` — L5 `exists = none`
- `docs/plan/flow/diagrams/S1.mmd:65-67` — OUT write inception + FOUNDATION.jsonl + jq readback
- `docs/contracts/s1_l3_walkthrough.md` — walkthrough; portal is not a second step list
- `docs/contracts/s1_l4_liveness.md` — `LiveVerdict` is an input field, not recomputed from tmux
- `docs/plan/FOUNDATION.jsonl` — stage=S1 output_refs name inception
- `mirror:beads_rust/src/sync/mod.rs:507-509` — `fsync_pinned_parent`
- `mirror:franken_lean/ci/CONVERGENCE_GOVERNANCE_POLICY.json` — gate/workstream/WIP step-ledger (governance analogue, not the portal schema)

## Work breakdown (filed, not claimed)

18 build (13 IDs + fsync-file + fsync-parent + rename + FOUNDATION append + decisions_owed) + 6 test. Gate `omp-orchestrator-gate-s1-l5-w44h` depends on each.

**Expectation collision:** this layer does **not** grow `expected_duration`. Journey `--delta` stays `UNMEASURABLE` until `omp-orchestrator-s1-expectation-perf-registry-xkr6` ships a duration-capable row. `NUMBERS.toml` `[figures.*]` cannot express duration. `decisions_owed.age_s` is age of a HD row, not an SLO. Two schemas would be worse than none — L5 defers.

| kind | id | title |
|---|---|---|
| build | `omp-orchestrator-s1-l5-schema-ciay` | L5-SCHEMA |
| build | `omp-orchestrator-s1-l5-hash-jbmy` | L5-HASH |
| build | `omp-orchestrator-s1-l5-source-van0` | L5-SOURCE |
| build | `omp-orchestrator-s1-l5-alert-cqwo` | L5-ALERT |
| build | `omp-orchestrator-s1-l5-one-next-qcev` | L5-ONE-NEXT |
| build | `omp-orchestrator-s1-l5-inception-y80i` | L5-INCEPTION |
| build | `omp-orchestrator-s1-l5-readback-3tek` | L5-READBACK |
| build | `omp-orchestrator-s1-l5-cursor-enjg` | L5-CURSOR |
| build | `omp-orchestrator-s1-l5-obs-schema-bbc8` | L5-OBS-SCHEMA |
| build | `omp-orchestrator-s1-l5-obs-hash-ub2l` | L5-OBS-HASH |
| build | `omp-orchestrator-s1-l5-obs-sources-1o28` | L5-OBS-SOURCES |
| build | `omp-orchestrator-s1-l5-obs-readback-zi3x` | L5-OBS-READBACK |
| build | `omp-orchestrator-s1-l5-metric-readback-fqgi` | L5-METRIC-READBACK-OK |
| build | `omp-orchestrator-s1-l5-fsync-file-q7jz` | fsync file |
| build | `omp-orchestrator-s1-l5-fsync-parent-u7qq` | fsync parent |
| build | `omp-orchestrator-s1-l5-rename-v809` | temp+rename |
| build | `omp-orchestrator-s1-l5-foundation-append-y6yg` | FOUNDATION.jsonl stage=S1 |
| build | `omp-orchestrator-s1-l5-decisions-owed-4tq2` | decisions_owed with age |
| test | `omp-orchestrator-s1-l5-test-envelope-wryz` | missing envelope field |
| test | `omp-orchestrator-s1-l5-test-one-next-rhro` | one_next object |
| test | `omp-orchestrator-s1-l5-test-readback-refuse-8vy2` | write 0 + readback fail => REFUSE |
| test | `omp-orchestrator-s1-l5-test-fsync-parent-nsyg` | parent fsync |
| test | `omp-orchestrator-s1-l5-test-hash-self-d8kk` | hash excludes self |
| test | `omp-orchestrator-s1-l5-test-crash-inject-p0jn` | crash between write and rename |

## NO-CLAIM

An envelope copied from ntm/am/bv is not a portal. `data_hash` on bv is 16 hex chars today, not SHA-256 of a portal row — copy the *role* (content hash), not the width. Parent-dir fsync is cited from beads_rust; this repo has no writer. `exists = none` remains.

---

Bar: Purpose, artifacts, ≥5 IDs, Validation, Cross-References, VIOLATION, Non-Coverage, NO-CLAIM, Bead line. No crate this wave.

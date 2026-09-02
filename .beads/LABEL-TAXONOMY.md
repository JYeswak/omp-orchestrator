# Bead label taxonomy — omp-orchestrator

Derived 2026-09-01 from the 119 distinct labels then in use across 142 beads
(`jq -r '.labels[]?' .beads/issues.jsonl | sort | uniq -c`), clustered against the
top-30 label vocabulary of the 166,757-bead / 150-repo corpus.

## Why this exists

Measured before consolidation: **119 distinct labels over 142 beads = 1 label per 1.2 beads**,
with **75 of 119 (63%) used exactly once**. The corpus runs **1,314 labels over 166,757 beads =
1 per 127**. At 1-per-1.2 a label is an adjective, not an index: `bv -l <label>
--robot-insights` scoping and the `bv --severity warning` alert surface — the navigation half of
the whole method — return one bead or zero. Consolidation restores scoping.

## Controlled taxonomy (allowlist)

This fenced block is the machine-readable allowlist. `crates/no-shell-gate/tests/bead_shape.rs`
parses it. **Do not duplicate this list anywhere in code.** One label per line, no comments.

```taxonomy
audit
coverage
correctness
decision
dispatch
extraction
gate
guardian
install
kernel
observability
omp
plan
rch
security
storage
testing
wiring
```

18 labels. Names match the corpus top-30 where the meaning matches (`testing`, `audit`,
`correctness`, `observability`, `security`); our real domain labels (`extraction`, `gate`,
`kernel`, `omp`, `coverage`) are legitimate and stay.

### What each label means

| label | scope |
|---|---|
| `extraction` | moving a crate out of control-plane into this repo, and the ordering/contract that governs it |
| `plan` | plan-space: `docs/plan/*` sections, denominators, provenance, convergence rounds, findings reconciliation, runbooks |
| `gate` | mechanical refusal surfaces: lints, ratchets, anti-vacuity legs, pre-commit/CI hooks, fail-closed behaviour |
| `kernel` | shared kernel crates and their interfaces (incl. the asupersync substrate contract), and bypasses of them |
| `omp` | OMP-surface coverage and OMP-version-specific behaviour |
| `coverage` | surface-coverage work and the numbered coverage waves |
| `dispatch` | the dispatch path: packet send, claim, ack, follow-up, lifecycle, coordination transport |
| `observability` | monitors and what they can see: liveness, pane truth, classifiers, logging, receipts |
| `wiring` | BUILT-but-NOT-WIRED: installing a proven mechanism where it actually runs, or retiring it |
| `testing` | conformance suites, differential oracles, hermetic test hygiene |
| `audit` | findings, censuses, inventories, grading debt, dogfood findings, architecture surveys |
| `correctness` | cancel-correctness, memory safety, ordering, process-group semantics, fixture drift |
| `security` | untrusted input reaching a shell or a parser |
| `guardian` | guardian/supervisor/orchestrator role ownership and fleet-wide sweeps |
| `rch` | remote compile host: admission, transfer, telemetry, capacity, upstream defects |
| `storage` | disk, retention, reapers, cargo target dirs, caches |
| `install` | installability from anywhere, identity proof, portability, release |
| `decision` | blocked on a named human decision (spend, retire, stop-condition) |

## Mapping — every one of the 119 original labels

`KEEP` = survives under its own name. A row with no target would be data loss; there are none.

| original | freq | → | note |
|---|---|---|---|
| extraction | 27 | KEEP `extraction` | |
| plan-derived | 22 | `extraction` (21) / `plan` (1) | provenance duplicated by the domain label. All 22 carriers are **tombstones and therefore frozen** — `br update` refuses tombstone mutation, so this label persists in the JSONL and is invisible to the gate (which excludes tombstones). The one non-extraction carrier is `dag-ready-gate-kwb` → `plan`. |
| gate | 18 | KEEP `gate` | |
| guardian | 17 | KEEP `guardian` | |
| p0 | 16 | **DROP → priority field** | duplicates `priority`. 15 of 16 carriers already have `priority: 0`. See "p0 discrepancy" below. |
| kernel | 15 | KEEP `kernel` | |
| omp | 13 | KEEP `omp` | |
| coverage | 12 | KEEP `coverage` | |
| wave | 11 | `coverage` | every carrier already had `coverage,omp`; purely redundant |
| converge | 11 | `plan` | |
| dispatch | 8 | KEEP `dispatch` | |
| plan | 7 | KEEP `plan` | |
| hd-0006 | 7 | `plan` | held-out-lens round id; round identity belongs in the title, not the index |
| convergence | 7 | `plan` | synonym of `converge`. Its last surviving carrier (`dag-ready-gate-kwb`) is a **tombstone and therefore frozen**, so this label persists in the JSONL and is invisible to the gate. |
| reconcile | 6 | `plan` | |
| rch | 6 | KEEP `rch` | |
| architecture | 6 | `audit` | every carrier is a survey/inventory/map of existing structure |
| wired-not-built | 5 | `wiring` | |
| tick-monitor | 5 | `observability` | crate name, not a category |
| shift-left | 5 | `gate` | |
| conformance | 5 | `testing` | |
| asupersync | 5 | `kernel` | asupersync **is** the kernel substrate here |
| wiring | 4 | KEEP `wiring` | |
| liveness | 4 | `observability` | |
| hd-0005 | 4 | `plan` | |
| dogfood-finding | 4 | `audit` | |
| ack | 4 | `dispatch` | |
| reaper | 3 | `storage` | |
| observability | 3 | KEEP `observability` | |
| gate-integrity | 3 | `gate` | |
| cancel-correctness | 3 | `correctness` | |
| anti-vacuity | 3 | `gate` | |
| wired | 2 | `wiring` | |
| testing | 2 | KEEP `testing` | |
| supervisor | 2 | `guardian` | |
| storage | 2 | KEEP `storage` | |
| runbook | 2 | `plan` | |
| no-shell | 2 | `gate` | |
| josh | 2 | `decision` | owner of the decision, not a category |
| inventory | 2 | `audit` | |
| gates | 2 | `gate` | plural synonym |
| disk | 2 | `storage` | |
| decision | 2 | KEEP `decision` | |
| capacity | 2 | `rch` | both carriers are rch capacity |
| workspace | 1 | `gate` | workspace-load gate |
| upstream | 1 | `rch` | |
| types | 1 | `kernel` | |
| transfer | 1 | `rch` | |
| supervision | 1 | `guardian` | |
| subprocess | 1 | `kernel` | |
| spend | 1 | `decision` | |
| shell | 1 | `security` | |
| runtime | 1 | `kernel` | |
| routing-safety | 1 | `dispatch` | |
| round-22 | 1 | `plan` | |
| retention | 1 | `storage` | |
| research | 1 | `plan` | |
| release | 1 | `install` | |
| receipt | 1 | `observability` | |
| ratchet | 1 | `gate` | |
| process-group | 1 | `correctness` | |
| process-debt | 1 | `audit` | |
| porting | 1 | `extraction` | |
| portability | 1 | `install` | |
| port | 1 | `extraction` | |
| pane-truth | 1 | `observability` | crate name |
| p1 | 1 | **DROP → priority field** | carrier already has `priority: 1` |
| ordering | 1 | `correctness` | |
| orchestrator | 1 | `guardian` | |
| omp-v18 | 1 | `omp` | |
| numbers | 1 | `plan` | |
| mission | 1 | `plan` | |
| mirror | 1 | `plan` | |
| memory-safety | 1 | `correctness` | |
| loop-enforcement | 1 | `gate` | |
| logging | 1 | `observability` | |
| lint | 1 | `gate` | |
| lifecycle | 1 | `dispatch` | |
| ledger | 1 | `plan` | |
| kernel-bypass | 1 | `kernel` + `gate` | the bead is a gate that detects kernel bypass |
| installable | 1 | `install` | |
| install | 1 | KEEP `install` | |
| injection | 1 | `security` | |
| infra | 1 | `rch` | |
| idle-capacity | 1 | `observability` | |
| identity | 1 | `install` | |
| hook | 1 | `gate` | |
| hermetic | 1 | `testing` | |
| grading | 1 | `audit` | |
| followup | 1 | `dispatch` | |
| fleet-wide | 1 | `guardian` | |
| fleet | 1 | `guardian` | |
| fixture-drift | 1 | `correctness` | |
| finding | 1 | `audit` | |
| fail-closed | 1 | `gate` | |
| evidence | 1 | `audit` | |
| dogfood | 1 | `audit` | |
| dispatch-safety | 1 | `dispatch` | |
| differential-oracle | 1 | `testing` | |
| decision-required | 1 | `decision` | |
| coordination | 1 | `dispatch` | |
| contract | 1 | `extraction` | tombstone-frozen (`815.1`); persists in JSONL |
| close-evidence | 1 | `gate` | |
| cleanup | 1 | `wiring` | "wire it or retire it" |
| classifier | 1 | `observability` | |
| claim-fence | 1 | `gate` | |
| ci | 1 | `gate` | |
| cargo | 1 | `storage` | |
| cache | 1 | `storage` | |
| build | 1 | `rch` | |
| blocked-on-human | 1 | `decision` | |
| blocked | 1 | `decision` | status, not a category |
| beads | 1 | `dispatch` | bead-comment ack is a dispatch-transport concern |
| authority | 1 | `plan` | |
| asupersync-conformance | 1 | `kernel` | |
| admission | 1 | `rch` | |
| S9 | 1 | `plan` | plan section id |
| S3 | 1 | `plan` | plan section id |
| S2 | 1 | `plan` | plan section id |

**119 rows. Zero unmapped.**

## p0 discrepancy — a real disagreement, NOT silently resolved

15 of the 16 `p0`-label carriers already carry `priority: 0`, so dropping the label loses
nothing. **`omp-orchestrator-xdx` carries the `p0` label while its `priority` field says `2`.**
Dropping the label therefore erases the only p0 assertion on that bead. Reprioritising was out
of scope for this pass, so the label was dropped and the disagreement is recorded here instead:

> `omp-orchestrator-xdx` — "rch worker-side reaper is default-OFF and aimed at /data/projects;
> arm it on /Users/josh/Developer" — asserted `p0` by label, `priority: 2` by field.
> **Decision owed:** set `priority 0`, or accept `2`.

## Ratchet

`crates/no-shell-gate/tests/bead_shape.rs` enforces a distinct-label ceiling, seeded from the
gate's OWN scan of non-tombstone beads immediately after this consolidation landed. The ceiling
may only fall. It is deliberately NOT seeded from the `jq` count above: a ratchet seeded from a
neighbouring measurement can sit one above the scan and let a mutation probe pass. The gate
prints its own count; that number is the seed of record.

## Adding a label

Adding a 19th label is a taxonomy change, not a bead edit: add it to the fenced `taxonomy`
block above with a scope row, and lower the ratchet in the same commit if the addition displaces
an existing label. If you find yourself adding a label for one bead, you are re-creating the
1-per-1.2 problem this document exists to end.

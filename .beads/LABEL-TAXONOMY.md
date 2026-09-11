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

## Plan axes (allowlist, second kind)

A plan-section axis is not a topical label. `s1` has **215 workable carriers** and `phase-0` has
20: these index, and `bv -l s1` groups. But an axis is a partition *declared a priori*, so its
thin members (`phase-7`, one carrier) are not the one-off defect the taxonomy cap exists to stop —
they are sections of a plan that has not reached them yet. Conflating the two is why a 25-label
topical cap kept fighting a legitimate 19-term partition.

So axes are declared separately, closed, and enumerated. `crates/no-shell-gate/tests/bead_shape.rs`
requires each member to be `<axis><n>` or `<axis>-<n>` (nothing else can ride in on an axis line),
caps the register at 3 axes / 12 members, and requires **each axis as a whole** to reach 2 members
in use and 8 carriers — an axis nobody uses is a smuggling route, and is refused.

```axes
s: s0 s1 s2 s5 s6
l: l0 l1 l2 l3 l4 l5
phase: phase-0 phase-1 phase-2 phase-3 phase-4 phase-5 phase-6 phase-7
```

Measured 2026-09-11 on workable beads: `s` 3 members in use / 217 carriers, `l` 6 / 190,
`phase` 8 / 27. Adding a plan section (`s3`) is a one-line edit here, reviewed like any
vocabulary change. Admitting these axes is **not** what makes the gate green: it moves
off-vocabulary workable beads from **531 to 530**, one bead, because a bead offends on any one of
several off-taxonomy labels. That measurement is the reason this gate polices the delta.

## Amnesty register — dated 2026-09-11, expires 2026-12-11, DOWN only

The 332 labels below were in use on live beads at the census (union of HEAD and the working tree,
so a dirty mirror cannot make the gate disagree with CI). They are **tolerated, not blessed**:

- No bead created on or after **2026-09-10** may carry any of them. Zero allowance. Measured:
  the 68 workable beads created 2026-09-10 and 2026-09-11 already carry none.
- A label **not** in this register and not in the taxonomy or an axis cannot appear on any bead at
  all. The register is a census, not a queue — it has no admission path.
- `AMNESTY_LABEL_CEILING` (332) and `ACTIVE_OFF_VOCABULARY_CEILING` (548 workable carriers at
  HEAD, 530 in the working tree) may only FALL.
- **Death condition:** past 2026-12-11 the gate fails unconditionally. Three months is the entire
  budget for draining these; renewal is a ruling, not a quiet constant edit.

```amnesty-labels
00
01
02
03
04
05
06
07
08
09
10
11
12
3xva-successor
L1
L4
L5
R1
S1
S4
S5
S6
S7
S9
a92y
abort
ack
ack-spine
actionability
admission
adoption
agent-mail
agent-mail-native
agent-work
agent_end
alignment
anti-vacuity
arc
architecture
arrow:S4-S5
arrow:S5-S6
artifact
artifact-lane
artifact-oracle
asupersync
atlas-arc
attribution
autonomous-loop
autonomy
beads
blocked-gauntlet
build
build-gate
built-not-wired
business-critical
bv
c112
capability-schema
cargo
census
ci
ci-shape
claim
claim-path
classification
classifier
cleanup
close-policy
command
commit-path
conductor
conformance
contention
continuity
continuous-operation
contract
contracts
control-kernel
control-plane
convergence
crate
crate:bead-lint
crate:crate-atom-gate
crate:lifecycle-arrow-gate
crates
cross-compile
cross-repo
dag
decision-ledger
declared-not-wired
definition
deletion
dependencies
derived-artifacts
dispatch-fence
dispatch-saga
doc
doc-fact
docs
doctrine
durability
entry-contract
epic
error-taxonomy
escalation
escaping
evidence
evidence-standard
execution-proof
exit-codes
f3g5-followon
fail-open
fairness
false-live
falsifier
figure-class
finding
findings
findings-ledger
fleet
fleet-block
fleet-blocking
fleet-wide
fooled-certificate
fsu7
fuzz
gate-layer
gate-rule-8
gate-runner
gates
get_state
golden
governance
governing
grade-pin
grading
grading-gate
grading-integrity
grading-lane
graph
green-tree
ground-truth
group-kill
handoff
hardening
harness
hd-0012
hd-0047
head-red
heartbeat
honest-credit
hook
hooks
human-decision
hygiene
idempotency
identity
idle-capacity
infra
install-parity
installability
installer
instrument
instrument-defect
journey
jsonl
kernel-contracts
kernel-math
kernel-only
key-mismatch
kxe
lane
lane-environment
latency
law
ledger
lifecycle
live-fire
liveness
loop-contract
loop-lifecycle
loop-queue-filter
loop-switch
m2
mail-health
materializer
measured
measurement
memory
messaging
metrics
milestone
mining
missing-representation
monitor
monitoring
mutation
mutation-did-not-bite
native
nndr-proof
no-shell
ntm
numbers-registry
obs
observation-contract
omp-surface
ompo
operability
operator-surface
oracle
orchestration
orphaned-claims
outage
owner-josh
owner-orchestrator
ownership
packet
pagerank
pane-observation
pane-truth
part:1
pass1-matrix
pass2
pass5
pass6
pass8
phase-gate
planning
platform-scope
policy
portability
preregistration
prescription:incarnation-lease-fencing
prescription:layered-enforcement-of-one-rule
prescription:saga-outcome-unknown-state
prescription:sampled-reexecution-trust-ladder
prescription:two-person-rule-transition-table
priority
probe
process-kernel
product
program
prompt
proven-by-landing
provenance
push-blocker
quality
r10
r3
r5
ratchet
readback
readiness
reaping
receipt
receiver-receipt
recurring-incident
redundancy
reference-integrity
refusal
registry
reliability
renderer
reservations
rigor-atlas
root-cause
rpc
rule-8d
safety
salvage
scheduler-kernel
scope
scope-control
scoping
scratch-home
selection
selector
self-fixture
self-referential
session
shared-checkout
silent-absence
silent-refusal
silent-success
skill-loop
skill-loop-pass3
skill-loop-pass4
skills
spike
stages-1-3
staleness
steer
strangling-edge
subprocess
subprocess-contract
substring-token
supervisor
surface-map
tech-debt
terminal
terminal-crates
test
test-coverage
test-quality
test-ratchet
tests
tmux
tracker
tracker-graph
transient
transport
tree-hygiene
truncation
trust-boundary
type-algebra
typed-artifacts
typed-no-data
types
unblock
unbounded-wait
upstream
vacuity
vacuous-absence
verb-wiring
verdict-honesty
verification
verification-hole
wave-a
wave-stability
wire
worktree-vs-index
writer
```

## Reserved — taxonomy entries with no carrier yet

`every_allowlist_label_is_actually_used` refuses an allowlist entry nobody uses, because an unused
entry is aspiration rather than vocabulary. A controlled vocabulary must still be allowed to
precede its first use, so the exceptions are named here individually rather than tolerated by a
count — `unused <= 1` hides *which* label is dead. The gate also refuses a reservation that has
come true, so these rows cannot become permanent.

```reserved
decision
```

`decision` has **zero carriers anywhere in the JSONL**, including tombstones, yet 82 beads are
`blocked`. Either the blocked beads are not blocked on a human, or they are unlabelled: that is a
real question this row keeps open instead of answering by deletion. **Death condition:** if
`decision` still has no carrier when the amnesty expires on 2026-12-11, delete the label.

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

`crates/no-shell-gate/tests/bead_shape.rs` enforces the registers above. Every ceiling is seeded
from the gate's OWN scan and may only fall. Seeds are deliberately NOT taken from the `jq` counts
in this document: a ratchet seeded from a neighbouring measurement can sit one above the scan and
let a mutation probe pass. The gate prints its own counts; those numbers are the seeds of record.

**Re-shaped 2026-09-11 from LEVEL to DELTA.** The off-taxonomy bead ceiling (526, re-recorded
2026-09-05) was breached six days later at 672. It was measured to be both population-coupled — it
rises when the fleet files work — and saturated: admitting 434 label uses of the `s*`/`l*`/`phase-*`
family moved it one bead. A ceiling that rises with productivity and will not fall under its own
remedy is a countdown, not a ratchet. What is enforced instead: zero tolerance on new beads,
identity-pinned vocabulary everywhere, a down-only grandfather register, and an expiry date.

## Adding a label

Adding a 19th label is a taxonomy change, not a bead edit: add it to the fenced `taxonomy`
block above with a scope row, and lower the ratchet in the same commit if the addition displaces
an existing label. If you find yourself adding a label for one bead, you are re-creating the
1-per-1.2 problem this document exists to end.

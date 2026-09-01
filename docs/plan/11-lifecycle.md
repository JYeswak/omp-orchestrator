# 11 — Lifecycle evidence map: idea to shipped, walked down the crates and skills

**R13, added by Josh mid-grading:** *"part of our plan needs to be intimately aware of the entire
lifecycle of an idea to a finished project then walk the list down the crates and our skills to
ensure that throughout dispatch we have proper templates, proper dispatch, proper reap, proper
logging, proper build grading, etc."*

This section is an evidence map for that spine. It is not itself the dispatchable runbook.

## Scope boundary: §12 owns the dispatchable runbook contract

The canonical journey is **§12, `docs/plan/12-journey.md`**. It defines the nine stage IDs
`S1`–`S9`, their artifacts, and the seven-field dispatch contract:

```text
### S<n> — <name>

Trigger.             What state means this stage is next.
Dispatch packet.     What an orchestrator sends a pane.
Amazing.             The fail-able quality bar.
Adequate.            The lower bar and its later cost.
Negative patterns.   Measured failure shapes.
Skills.              Skills used and not used.
Done signal.         Artifact, proof command, and exit code.
```

Those seven fields intentionally are **not duplicated in this analysis section**. §11 cross-
references §12 rather than pretending to be a second runbook; applying the runbook contract to
this file is a **scope error**. The transition tables below still record measured inputs,
outputs, refusals, and current boundaries so that §12 can be implemented against evidence.

All claims below are `MEASURED` unless explicitly marked `PROJECTED`, `DECLARED`, `WIRE-PROVEN`,
`NOT CONSUMED`, or `NO-CLAIM`.

---

## 11.1 Canonical stage graph and current ownership

The nine names and IDs below are copied from §12. The old words *idea*, *plan*, *bead*, *select*,
*dispatch*, *work*, *reap*, *grade*, and *ship* are useful **operational subphases**, but are not
additional `S` IDs. `viability`, `loop`, and `honesty` are cross-stage attributes, not stages.

| canonical ID | stage (the §12 name) | operational subphases represented here | accountable owner / consumer | observer | current crate state |
|---|---|---|---|---|---|
| `S1` | **Inception** | idea intake and viability | human decision owner; `/idea-wizard` and `/product-viability-gauntlet` are process consumers | none identified | no dedicated crate; process artifact only |
| `S2` | **Planning** | plan authoring | plan author; `/planning-workflow` | none identified | no dedicated crate; this plan is a document |
| `S3` | **Grading the plan** | independent plan grading | independent grader; no single local crate was identified | none identified | no shared grade value is measured |
| `S4` | **Beads DAG** | bead creation, dependency closure, ready selection, graph ranking | `loop-queue-filter` is the intended local consumer of `bv`; `br` remains an external tracker | none identified | `loop-queue-filter` exists, but focused search found no `bv` invocation |
| `S5` | **Execution** | claim, dispatch, worker work, receiver receipt | **`omp-orchestrator` is the resident accountable consumer**; its manifest supports `dispatch-claim-fence`, `ack-stage`, `omp-rpc-session`, `subprocess-contract`, and `receiver-receipt` | `tick-monitor` is observation-only and is consumed by the resident supervisor | automated observe → queue → dispatch → receipt path is designed; runtime proof is absent; `pane-dispatch-fence` exists in the inventory but is not proven a resident dependency |
| `S6` | **Grading the work** | receipt review and independent grade | `ack-stage` exists; `ack-spine` contains a follow-up candidate; no `verify-dispatch` crate exists | none identified | grading remains non-shared/prose-shaped |
| `S7` | **Validation** | completion/reap and external validation | no current reap owner; `omp-rpc-session` is transport-only, not a completion consumer | `tick-monitor` can observe panes, but observation is not reap | `reap-finished-panes` is absent; completion consumer is not wired |
| `S8` | **Ship** | release, build, install, rollback | `installer` and `commit-build-fence`; /installer-workmanship and /release-preparations | none identified | **DECLARED (R10):** installer covers 3 of 18 binaries; the denominator's inventory/counting rule was not preserved; foreign-host `--install` is unverified |
| `S9` | **Human requirements stored** | decision capture and retrieval, cross-cutting across S1–S8 | human is the decision owner; **PROJECTED** append owner: `omp-orchestrator` | every stage must observe its own decision handoff | no dedicated S9 consumer is proven; `ack-spine::ledger` is not proven to be this ledger |

### Current crate names versus projected names

The former map called several names current owners without checking the current inventory. The
focused R10 search found no current crates named `fast-dispatch`, `tick-dispatch`,
`reap-finished-panes`, or `verify-dispatch`. Those names are **PROJECTED / unbuilt**, not owners.
Conversely, `crates/omp-orchestrator/Cargo.toml:16-23` lists `ack-stage`,
`dispatch-claim-fence`, `omp-rpc-session`, `subprocess-contract`, and `receiver-receipt`, and
`crates/omp-orchestrator/src/main.rs:563-631,801-879` implements the resident dispatch/receipt
path. `pane-dispatch-fence` is present in the crate inventory but is not listed as an
`omp-orchestrator` manifest dependency in the measured slice. Ownership claims therefore name
`omp-orchestrator` as the S5 consumer and list only the manifest dependencies as supporting
edges; a separate current crate is not thereby proven called in production.

The graph represented by this section is therefore:

```text
S1 Inception → S2 Planning → S3 Grade plan → S4 Beads DAG
→ S5 Execution → S6 Grade work → S7 Validation → S8 Ship
                         ↘ S9 Human requirements stored (cross-cutting)
```

`S9` is not a ninth sequential finish step. **PROJECTED REQUIREMENT:** every arrow and every
stage must emit a decision record for S9; no current emitter or consumer was measured.

---

## 11.2 The five R13 properties, measured against the canonical stages

For this matrix, `Y` means a distinct mechanism exists **and was used in the measured session**;
`y` means it exists but was not used; `—` means absent; `n/a` means the property is not
meaningful for that stage; and `↗ S<n>` means the evidence is shared with another stage and is
not a second observed cell.

| canonical stage | template | dispatch | reap | logging | build grading |
|---|:--:|:--:|:--:|:--:|:--:|
| `S1` Inception | — | — | — | — | — |
| `S2` Planning | — | — | — | — | — |
| `S3` Grading the plan | — | n/a | — | — | — |
| `S4` Beads DAG | — | n/a | — | `Y` (`.beads/issues.jsonl`) | — |
| `S5` Execution | `y` | `y` (resident observe → queue → dispatch → receipt path; source/design evidence only) | n/a | partial | n/a |
| `S6` Grading the work | — | n/a | n/a | `Y` (bead comments) | `Y` |
| `S7` Validation | — | n/a | `y` (candidate only) | — | n/a |
| `S8` Ship | — | — | n/a | — | `↗ S6` (shared build evidence; not an independent `Y`) |
| `S9` Human requirements stored | — | n/a | n/a | — | n/a |

There are **three distinct visible `Y` cells out of 45**: S4 logging, S6 logging, and S6 build
grading. S5 dispatch is deliberately `y`, because the resident path is source/design evidence
only; R10 did not runtime-verify it. This is the known LIFE-09 reinforcement of R8 `m-l6`, not a
new finding. The `S8` alias is deliberately not counted twice. The count says only that those
three observations occurred; it does not say that the nine-stage journey completed.

Human actuation remains a prerequisite for an unclaimed bead. The resident supervisor's later
dispatch path is a designed/source-level property, not measured runtime use.

---

## 11.3 Template omission refusal is not claim-custody refusal

`ntm template list` measured four templates, including the `dispatch` template:

```text
Name:        dispatch
Description: ZestStream controller dispatch packet — bounded assignment with proof obligations
             and a named …
Path:        /Users/josh/.config/ntm/templates/dispatch.md
Variables:
  - objective (required)   ONE outcome, stated as a result not an activity
  - target    (required)   Absolute repo/worktree path, and the bead ID
  - why_now
```

The template's required variables protect **packet shape**. They do not prove tracker custody.
`dispatch-claim-fence/src/lib.rs:257-319` authorizes a bead only from a fresh snapshot whose
status is `in_progress` and whose assignee exactly matches the receiver. A `target` string that
contains a path and bead ID cannot establish that state.

The two historical packets (`5rh` → `%1413` and `omp-coverage-mission-ipg.4`) demonstrate the
missing middle beat: `select → claim → dispatch`. The target was hand-written into `/tmp` and
sent with `tmux send-keys -l`; the template was not used. The template's body was not tested in
R10, so there is **NO-CLAIM** that its body itself catches either packet defect.

### Required refusal probes, kept as separate contracts

The following are the exact probes the future runbook must execute. Expected refusal text and
exit are **PROJECTED** unless marked otherwise; R10 captured no omitted-variable stderr/exit
artifact.

| contract | input | expected fail-closed result |
|---|---|---|
| template omission | `ntm send -t dispatch --var target=/abs/repo:BEAD --dry-run` (omit required `objective`) | nonzero template refusal naming the missing required variable; **PROJECTED, not captured** |
| template omission | `ntm send -t dispatch --var objective=outcome --dry-run` (omit required `target`) | nonzero template refusal naming the missing required variable; **PROJECTED, not captured** |
| absent tracker snapshot | `authorize(DispatchIntent::Bead { bead_id, receiver_agent }, None)` | `MissingSnapshot`, rendered as `DISPATCH_BLOCKED … tracker snapshot is missing` |
| open/unassigned tracker row | matching snapshot with status `open` and no assignee | `ClaimRequired`, with `br update <bead> --assignee <receiver> --status in_progress` |
| closed, blocked, deferred, or unknown row | matching snapshot with that status | `ClaimRequired` or `UnknownStatus`; never a dispatch permit |
| assigned elsewhere | matching `in_progress` snapshot with another assignee | `AssignedElsewhere`, with a `DISPATCH_BLOCKED` refusal |
| snapshot for another bead | requested ID differs from snapshot ID | `SnapshotIdMismatch`, with a `DISPATCH_BLOCKED` refusal |

The claim-fence is therefore a **custody verifier**, not a claim creator. Its refusal cannot be
replaced by a template-variable check.

---

## 11.4 The build-grading hook is a shell script, and the rule cannot see it

The repo's one hard rule is **no `.sh`, no `.py`**, enforced by `no-shell-gate` with an empty
exemption list. R10 measured:

```text
ls -la .git/hooks/*.sh
  .git/hooks/commit-msg-verification-level.sh   6288 bytes

git ls-files | grep -c commit-msg-verification
  0
```

The script that enforces build-grading discipline is a 6.3 KB shell script, invisible to the
rule because `no-shell-gate` scans the git index. The gate states its boundary at
`crates/no-shell-gate/src/lib.rs:14`: *"this gate covers FILE EXTENSIONS of tracked files,
nothing else."* This is a coverage finding, not a claim that the gate implementation is
incorrect. Q13 remains unresolved and is retained here rather than silently closed:

1. declare `.git/hooks` legitimately outside the rule because hooks are machine-local;
2. replace the hook with a Rust binary like the other gates; or
3. record a named allowance, owner, and reason.

The lifecycle section does not choose among those policy decisions. **NO-CLAIM:** Q13 is retained
for its owner and policy decision; no new exemption or migration is asserted here.

---

## 11.5 Selection → claim → dispatch, and every measured downstream break

The former text called this "three severed links" while naming only two arrows. The reconciled
count is **four sequential breaks plus one cross-cutting S9 ledger handoff**. A break means that
the next-stage artifact is not produced or consumed by the current production path; it does not
mean that no partial mechanism exists.

| edge | required input | expected handoff artifact | refusal / non-terminal rule | measured current state |
|---|---|---|---|---|
| `S4 → S5` | graph-selected bead, fresh `br show --json`, receiver | claim record, then dispatch-template packet and permit | refuse missing/open/elsewhere-assigned snapshot; do not send before claim | `main.rs:836-860` runs `br ready`, takes `bead_ids.first()`, reads a snapshot, and calls `authorize`; no production claim/update caller was found. **Human claim required today; atomic handoff PROJECTED.** |
| `S5 → S6` | dispatch attempt, receiver receipt, session/pane identity | grade packet tied to the receipt and bead | refuse absent receiver receipt; receipt is not a grade | resident path reaches receipt and stops; no production grade handoff is wired |
| `S6 → S7` | independent grade plus worker completion evidence | validation/reap input | an in-progress or non-terminal completion is not finished | completion frame is wire-proven, but local parser/consumer/reap are absent |
| `S7 → S8` | validation result, external/foreign-host run evidence | ship/release packet with rollback | refuse without validation artifact or rollback path | no production validation-to-ship edge is measured |
| `S8 → S9` | ship decision and human choice | append-only S9 decision record | refuse missing decision owner, decision, or retrieval key | S9 ledger owner and retrieval path are **PROJECTED**, not wired |

The current contract is therefore explicit: until an atomic claim owner is implemented, a human
MUST run the claim command and the fence MUST read back the resulting `in_progress` row before
S5 dispatch. The future atomic wrapper is a `PROJECTED` remedy, not a current capability.

---

## 11.6 S5 completion boundaries and the reap consumer

### S5 is automated through receipt, not through a proven full journey

`crates/omp-orchestrator/src/main.rs:563-631,801-879` and `src/lib.rs:24-29` describe an automated
observe → queue → dispatch → receiver-receipt path. That corrects the old human-only S5 claim,
but it does not prove runtime behavior on the live fleet. The current local `omp-rpc-session`
crate is explicitly a transport for **one** `--mode=rpc` child (`crates/omp-rpc-session/src/lib.rs:5-21`)
and does not claim cross-session continuity.

Completion evidence has five separate layers; they must not be collapsed:

| layer | evidence | status |
|---|---|---|
| declaration | upstream `AgentEndEvent` at `dist/types/extensibility/shared-events.d.ts:154`, with `willContinue` | **AVAILABLE / DECLARED** |
| wire observation | `%1408` / `%1414`; `/tmp/grade/agent-end-raw-frame.json` contains `{"type":"agent_end","isTerminal":true}` | **WIRE-PROVEN for one terminal frame** |
| local parser | `omp-rpc-session/src/lib.rs:416-423` recognizes only Ready/Response/Unknown/Malformed | **NOT IMPLEMENTED for AgentEndEvent** |
| local consumer | focused search found no `agent_end`, `willContinue`, `isTerminal`, `RpcSessionEventFrame`, or `AgentEndEvent` consumer | **NOT CONSUMED** |
| reap | no production completion consumer and no current `reap-finished-panes` crate | **NOT WIRED** |

`isTerminal: true` is not proven equivalent to `willContinue: false`; one terminal frame cannot
establish non-terminal settle behavior, crashes, killed panes, rate-limited turns, or compaction.
The honest status is therefore **completion AVAILABLE and WIRE-PROVEN, but NOT CONSUMED locally**.
The work moved from inventing a protocol to adopting an existing event plane, but adoption still
requires changing the one-child attachment topology. No completion crate is claimed.

### Reap is a consumer, not an idle observation

The named `reap-finished-panes` crate is absent. `ack-spine/src/followup.rs:86-137` is a pure
candidate classifier, and the focused `classify_followup|followup_action` search found no
production caller. It has a measured false-completion path:

* for an open/in-progress bead, unchanged assignee, no comment, and before the deadline,
  `classify_followup` returns `FollowUpVerdict::VerdictPosted`; and
* `followup_action` maps `VerdictPosted` to `Healthy` at `followup.rs:150-156`.

That state is **in progress**, not a posted verdict and not a finish. A future consumer MUST
represent it as a distinct non-terminal `InProgress` result. Only a read-back closed row may
produce `Finished`; only `Finished` may authorize refill. `SilentPastDeadline` remains a
follow-up, not a refill. These are **PROJECTED contract repairs**, not claims that the current
candidate has been changed.

The resident cycle (`main.rs:648-687,801-880`) stops after dispatch/receipt. It has no production
reap → grade → validation → ship edge. This is the explicit post-dispatch **NO-CLAIM** boundary
for the current supervisor.

### Settled wire fact

The `%1414` result remains useful and is not withdrawn: `AgentEndEvent` crosses `--mode=rpc` in
the captured terminal case. It closes the claim that OMP has no completion precedent, but it does
not close the adoption, parser, consumer, or reap claims above.

---

## 11.7 Surface-map counts: measured universe and current WIRE cardinality

R14/R15 batch rows 1–9 contain **270 mapped rows** across `ntm`, `br`, `bv`, and OMP. The
**544-row R14/R15 surface-universe denominator is `DECLARED` from the R14/R15 review** rather
than derived from this section; `NUMBERS.toml` records related surface-map snapshot drift and figure discipline. The named
frozen input for the reproducible scoped count is `docs/plan/SURFACE-MAP.jsonl`, SHA-256
`f155a358dd302982367a7c0107fe0eb1e3cd6f5ec7d4689bac67f11b1c5063f7`; that snapshot currently
contains 591 rows. The disposition policy is exact: include rows whose `batch` is a JSON number
equal to its floor and in 1 through 9 inclusive, then group by the literal `disposition` field.
Both count commands below use that same frozen input and explicit integer predicate.

```sh
SNAPSHOT=docs/plan/SURFACE-MAP.jsonl
printf 'snapshot_sha256 '; shasum -a 256 "$SNAPSHOT"
jq -s '
  def in_scope:
    .batch as $b
    | if ($b|type) == "number"
      then (($b == ($b|floor)) and $b >= 1 and $b <= 9)
      else false
      end;
  {
    surface_universe: length,
    scoped_integer_1_9: (map(select(in_scope)) | length),
    excluded_batch_type: (map(select((.batch|type) != "number")) | length),
    excluded_non_integer_batch:
      (map(select(.batch as $b
        | if ($b|type) == "number" then $b != ($b|floor) else false end)) | length),
    excluded_numeric_out_of_range:
      (map(select(.batch as $b
        | if ($b|type) == "number"
          then (($b == ($b|floor)) and ($b < 1 or $b > 9))
          else false
          end)) | length),
    by_disposition: (map(select(in_scope))
      | group_by(.disposition)
      | map({disposition: .[0].disposition, count: length}))
  }' "$SNAPSHOT"
```

The measured result (exit 0) is:

```text
snapshot_sha256 f155a358dd302982367a7c0107fe0eb1e3cd6f5ec7d4689bac67f11b1c5063f7  docs/plan/SURFACE-MAP.jsonl
{
  "surface_universe": 591,
  "scoped_integer_1_9": 270,
  "excluded_batch_type": 0,
  "excluded_non_integer_batch": 0,
  "excluded_numeric_out_of_range": 321,
  "by_disposition": [
    {"disposition": "CONSUMED", "count": 8},
    {"disposition": "RETIRE", "count": 214},
    {"disposition": "UNPROBEABLE-PENDING", "count": 6},
    {"disposition": "VALIDATE", "count": 11},
    {"disposition": "WIRE", "count": 31}
  ]
}
```

The old `RETIRE 243 / WIRE 11 / VALIDATE 8` totals were stale. The earlier 11-row routing
excerpt was illustrative, **not the WIRE universe**. The statement "the value is in the 11"
is withdrawn; there are 31 WIRE proposals and they must be treated as proposals until wired.

A grouped WIRE derivation using the same frozen input and predicate is:

```sh
SNAPSHOT=docs/plan/SURFACE-MAP.jsonl
jq -s '
  def in_scope:
    .batch as $b
    | if ($b|type) == "number"
      then (($b == ($b|floor)) and $b >= 1 and $b <= 9)
      else false
      end;
  [.[] | select(in_scope and .disposition == "WIRE")]
  | group_by(.maps_to_crate // "UNASSIGNED")
  | map({crate: (.[0].maps_to_crate // "UNASSIGNED"), count: length})' \
  "$SNAPSHOT"
```
The measured grouping is:

| current beneficiary | WIRE rows |
|---|---:|
| `omp-orchestrator` | 18 |
| `loop-queue-filter` | 7 |
| `installer` | 4 |
| `fleet-composite` | 1 |
| `tick-monitor` | 1 |
| **total** | **31** |

The seven selection-related WIRE rows (`br:blocked`, `br:dep`, `bv:candidates`,
`bv:decision-relevant`, `bv:dependencies`, `bv:not-ready`, `bv:robot`) still point to
`loop-queue-filter`, supporting it as the intended S4 graph consumer. The 18 rows pointing to
`omp-orchestrator` are the larger current WIRE cluster and include `ntm:template` plus other
resident-control-plane surfaces. Neither convergence result proves implementation or schedule.

The eight VALIDATE rows remain a dependency warning: `br:close`, `br:create`, `br:init`,
`br:list`, `br:schema`, `br:sync`, `br:update`, and `bv:exit-codes` rely on external behavior
without a local assertion. A `VALIDATE` disposition is not a passing test.

**NO-CLAIM:** the 544-row R14/R15 denominator remains the declared review figure, not a claim that
every later row or every future surface is included. The hash above identifies only the current
`SURFACE-MAP.jsonl` snapshot; future updates MUST freeze an immutable JSONL snapshot before deriving
counts. A WIRE row names a proposed beneficiary, not a completed integration.

---

## 11.8 Skills are facets of the canonical stages, not twelve extra stages

The R10 `jsm search` output was **declared** as having 12 operational rows, 18 skill references,
and 16 unique skill names; the raw output and counting derivation were not preserved. That is a
skill/facet inventory, not a second stage graph. The canonical mapping is:

| canonical stage or attribute | skill references | boundary |
|---|---|---|
| `S1` Inception | `/idea-wizard`, `/dueling-idea-wizards`, `/brainstorming` | prose ideation; no durable typed output by itself |
| `S1` viability attribute | `/product-viability-gauntlet` | fail-closed kill/narrow/pilot/build verdict; not an inception artifact |
| `S2` Planning | `/planning-workflow` | markdown plan; convergence is judged by review |
| `S2` loop attribute | `/loop-engineering` | verified-value tick loop; not a new stage |
| `S4` Beads DAG | `/beads-workflow`, `/beads-north-star`, `/beads-br`, `/beads-bv` | tracker schema, close policy, and graph ranking; local `bv` consumption is absent |
| `S5` Execution | `/ntm`, `/vibing-with-ntm` | robot surfaces and operator doctrine; local completion/reap adoption is absent |
| `S6` Grading the work | `/beads-compliance-and-completion-verification` | prose verdicts; no shared grade value |
| `S7` Validation | `/vibing-with-ntm` | observation and tending; not a production reap consumer |
| `S8` Ship | `/installer-workmanship`, `/release-preparations` | installer/release process; foreign-host install proof remains absent |
| `S9` decision attribute | `/just-say-no-to-process-porn-and-ceremony` | honesty lens, not a decision ledger |

The prior `S1.5`, `S2.5`, and `S8.5` labels are now explicitly attributes. They must not be
reused as stage IDs, and they do not conflict with §12's S1–S9.

A stage is **typed** only when a downstream stage can consume its output as a value without a
human reading prose. The measured typed boundaries are narrow:

* `br` supplies a typed bead row for S4, and `bv` declares a typed ranking contract, but the
  local `loop-queue-filter` consumer is not wired;
* S5 has a receiver receipt mechanism, but `omp-orchestrator` does not consume a local
  completion event type;
* S6 has six Verdict-shaped types across the repo with no shared trait, so its result is not one
  countable value; and
* S7 has an upstream completion frame but no local parser/consumer/reap.

`omp-types` has zero dependents and is a possible future home for shared handoff types. The
upstream `IrcDeliveryReceipt` declaration (`tools/hub/types.d.ts:8`) and `AsyncJobDeliverySink`
(`:84`) remain **DECLARED only** and are not evidence that this local S5 path consumes them.

**NO-CLAIM:** this maps the R10 search results; it does not prove that the 16 skills are the only
skills that could participate, or that they compose cleanly merely because they are named here.

---

## 11.9 Stage logging and the S9 decision ledger

Current heartbeat rows at `crates/omp-orchestrator/src/main.rs:698-708` contain `event`, `status`,
`tick`, `repo`, `session`, and `detail`. Focused search found no `stage_id`, `from_stage`, or
`to_stage`. The old "3 of 9 stages log" statement is withdrawn as a stage-level guarantee: a
few files contain records, but those records cannot prove a stage transition.

The required **PROJECTED** append-only lifecycle event shape is:

```json
{
  "schema_version": 1,
  "event_id": "unique-within-session",
  "session_id": "session-name",
  "stage_id": "S5",
  "from_stage": "S4",
  "to_stage": "S5",
  "command": "br update BEAD --assignee AGENT --status in_progress",
  "exit_code": 0,
  "artifact": ".omp/lifecycle-events.jsonl",
  "status": "CLAIMED",
  "observed_at": "RFC3339"
}
```

The append target MUST be session-scoped and append-only. The event must record the command,
exit, artifact path, and canonical stage IDs; `detail` alone is insufficient. The current
heartbeat schema does not satisfy this shape and is not being represented as if it did.

S9's minimum decision record is also **PROJECTED**:

```json
{
  "decision_id": "unique-within-session",
  "session_id": "session-name",
  "stage_id": "S8",
  "owner": "human-operator",
  "question": "ship, hold, or rollback?",
  "decision": "HOLD",
  "decided_at": "RFC3339",
  "evidence_artifact": "relative/path",
  "conditions": ["foreign-host install proof pending"]
}
```

The projected accountable append owner is `omp-orchestrator`; the human remains the decision
owner. The projected retrieval command is:

```sh
jq -c 'select(.stage_id == "S9" or .decision_id != null)' \
  "$OMP_LIFECYCLE_EVENTS" \
  | jq -c 'select(.session_id == env.OMP_SESSION)'
```

No such file, owner wiring, or retrieval output was measured in R10. Pane scrollback contained
human decision activity, but no pane/session artifact or extraction/count procedure was preserved;
therefore no numeric count is claimed. This is evidence of decision loss, not evidence of a ledger.

---

## 11.10 One-to-many namespace and cardinality contract

The current implementation is not a proven 1:many orchestrator. Its source/configuration facts and
behavior are listed below; no captured multi-session runtime probe is presented, so `observed` is
reserved for a captured runtime probe.

| surface | source-derived/static behavior | safe current contract |
|---|---|---|
| resident `omp-orchestrator` process | one configured supervisor process | one process per session until fan-out is proven |
| `omp-rpc-session` | exactly one OMP `--mode=rpc` child; no cross-session continuity | one child per attached session; no cross-session completion claim |
| pane candidates | `lib.rs:447-462,494-499` counts dispatchable panes but returns `.first()` | one selected pane per cycle; `N > 1` must not silently truncate |
| ready beads | `main.rs:854-862` selects `bead_ids.first()` | one selected bead per cycle; `N > 1` must not silently truncate |
| heartbeat ledger | session-named path exists, but tick-monitor state and pending-dispatch are fixed filenames (`main.rs:221-236`) | fixed paths are a collision; refuse a second owner or use per-session keys |
| claim permit | one bead ID plus one receiver in `DispatchIntent::Bead` | one bead → one receiver → one permit |
| completion/reap | no local AgentEndEvent consumer and no reap producer | zero automatic refill claims until a consumer is wired |

Until the namespace repair is implemented, the honest support boundary is:

```text
1 process : 1 session : 1 OMP child : 1 selected bead : 1 selected pane : 1 receipt
```

A request that observes more than one candidate MUST produce a typed `CARDINALITY_REFUSED`
(or an equivalent explicit human decision) rather than taking `.first()` silently. A second
session in the same HOME MUST refuse when it would reuse a fixed state or pending-dispatch path.
Per-session keys, collision detection, and bounded fan-out are **PROJECTED**; no 1:many runtime
proof is claimed.

---

## Closing boundary

The A-to-Z process is now named without inventing a second graph: canonical runbook stages are in
§12; this section supplies the measured crate ownership, transition gaps, template/fence split,
completion/reap boundary, surface-map counts, logging/S9 schema, skill facets, and cardinality
limit that §12 must honor.

**NO-CLAIM.** This section does not establish that the current journey ships software unattended.
It establishes where the current resident path stops, which upstream completion fact is reachable,
which local consumers are absent, which records are durable or not, and which 1:many behaviors are
explicitly refused until proven.

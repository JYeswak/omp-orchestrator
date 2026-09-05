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

All claims below are `MEASURED` unless explicitly marked `PROJECTED`, `DECLARED`,
`WIRE-PROVEN`, `NOT CONSUMED`, or `NO-CLAIM`. R13 is represented by the stage/property matrix below, and OQ-13 remains an unresolved policy choice in §11.4; §00 is outside this assignment and is not treated as a second authority.

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
| `S6` | **Grading the work** | receipt review and independent grade | `ack-stage` exists; `ack-spine` contains a follow-up candidate; no shared grade consumer is proven | none identified | grading remains non-shared/prose-shaped; `verify-dispatch` exists but is not proven the S6 consumer |
| `S7` | **Validation** | completion/reap and external validation | `reap-finished-panes` is invoked by the resident supervisor for the finished-pane sweep; no local AgentEndEvent completion consumer | `tick-monitor` can observe panes, but observation is not completion reap | finished-pane sweep is wired; completion consumer and completion-based refill are not wired |
| S8 | **Ship** | release, build, install, rollback | installer and commit-build-fence; /installer-workmanship and /release-preparations | none identified | **DECLARED (NUMBERS.toml):** installer knows 3 of 48 current binary targets; the denominator/counting rule is registry-backed; foreign-host --install is unverified |
| `S9` | **Human requirements stored** | decision capture and retrieval, cross-cutting across S1–S8 | human is the decision owner; **PROJECTED** append owner: `omp-orchestrator` | every stage must observe its own decision handoff | no automated S9 writer/consumer is proven; `docs/decisions.jsonl` has manually recorded HD rows, not proof of wiring |

### Current crate names versus projected names

The former map called several names current owners without checking the current inventory. The current inventory confirms `fast-dispatch`, `tick-dispatch`, `reap-finished-panes`, and `verify-dispatch` as crates. Their existence does not prove production ownership or a complete stage handoff. The names are therefore **CURRENT CRATES / UNPROVEN CONSUMERS**, not projected or absent.

Conversely, `crates/omp-orchestrator/Cargo.toml:Conversely` lists `ack-stage`, `dispatch-claim-fence`, `omp-rpc-session`, `subprocess-contract`, `receiver-receipt`, and other supporting dependencies. The resident dispatch path is in `crates/omp-orchestrator/src/main.rs:Conversely`; the finished-pane sweep helper is at `main.rs:Conversely`. `pane-dispatch-fence` is present in the crate inventory but is not listed as an `omp-orchestrator` manifest dependency in the measured slice. Ownership claims therefore name `omp-orchestrator` as the S5 consumer and list only manifest dependencies as supporting edges; a separate current crate is not thereby proven called in production.

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
| `S7` Validation | — | n/a | `y` (finished-pane sweep exists and is invoked; completion-event reap remains absent) | — | n/a |
| `S8` Ship | — | — | n/a | — | `↗ S6` (shared build evidence; not an independent `Y`) |
| `S9` Human requirements stored | — | n/a | n/a | — | n/a |

There are **three distinct visible `Y` cells out of 45**: S4 logging, S6 logging, and S6 build
grading. S5 dispatch is deliberately `y`, because the resident path is source/design evidence
only; R10 did not runtime-verify it. This is the known LIFE-09 reinforcement of R8 `m-l6`, not a
new finding. The `S8` alias is deliberately not counted twice. The count says only that those
three observations occurred; it does not say that the nine-stage journey completed.

Human actuation remains a prerequisite for an unclaimed bead. The resident supervisor's later
dispatch path is a designed/source-level property, not measured runtime use.

> *Upstream type for this gap: `Stage1Claim`/`ownershipToken` (`memories/storage.d.ts:ownershipToken`, DECLARED only). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

---

## 11.3 Template omission refusal is not claim-custody refusal

**HISTORICAL SNAPSHOT:** ntm template list reported four templates including dispatch. The current read-only command ntm template list --json | jq 'if type=="array" then length elif .templates then (.templates|length) else empty end' returns **16** templates, including dispatch; the excerpt below is retained as the packet-shape example, not a current inventory.

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
`dispatch-claim-fence/src/lib.rs:heading_dispatch_claim_fence_src_lib_rs_257` authorizes a bead only from a fresh snapshot whose
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
`crates/no-shell-gate/src/lib.rs:EXTENSIONS`: *"this gate covers FILE EXTENSIONS of tracked files,
nothing else."* This is a coverage finding, not a claim that the gate implementation is
incorrect. OQ-13 remains unresolved and is retained here rather than silently closed:

1. declare `.git/hooks` legitimately outside the rule because hooks are machine-local;
2. replace the hook with a Rust binary like the other gates; or
3. record a named allowance, owner, and reason.

The lifecycle section does not choose among those policy decisions. **NO-CLAIM:** OQ-13 is retained
for its owner and policy decision; no new exemption or migration is asserted here.

---

## 11.5 Selection → claim → dispatch, and every measured downstream break

The former text called this "three severed links" while naming only two arrows. The reconciled
count is **four sequential breaks plus one cross-cutting S9 ledger handoff**. A break means that
the next-stage artifact is not produced or consumed by the current production path; it does not
mean that no partial mechanism exists.

| edge | required input | expected handoff artifact | refusal / non-terminal rule | measured current state |
|---|---|---|---|---|
| `S4 → S5` | graph-selected bead, fresh `br show --json`, receiver | claim record, then dispatch-template packet and permit | refuse missing/open/elsewhere-assigned snapshot; do not send before claim | `main.rs:prepare_bead_dispatch` runs the finished-pane sweep and `br ready`, takes `bead_ids.first()`, then `prepare_bead_dispatch` at `main.rs:prepare_bead_dispatch` claims open rows and calls `authorize`; no separate atomic claim service is proven |
| `S5 → S6` | dispatch attempt, receiver receipt, session/pane identity | grade packet tied to the receipt and bead | refuse absent receiver receipt; receipt is not a grade | resident path reaches receipt and stops; no production grade handoff is wired |
| `S6 → S7` | independent grade plus worker completion evidence | validation/reap input | an in-progress or non-terminal completion is not finished | completion frame is wire-proven, but local parser/consumer/reap-by-completion are absent |
| `S7 → S8` | validation result, external/foreign-host run evidence | ship/release packet with rollback | refuse without validation artifact or rollback path | no production validation-to-ship edge is measured |
| `S8 → S9` | ship decision and human choice | append-only S9 decision record | refuse missing decision owner, decision, or retrieval key | S9 ledger writer and retrieval path are **PROJECTED**, while three manual HD rows exist |

The current contract is therefore explicit: until an atomic claim owner is implemented, a human
MUST run the claim command and the fence MUST read back the resulting `in_progress` row before
S5 dispatch. The future atomic wrapper is a `PROJECTED` remedy, not a current capability.

---

## 11.6 S5 completion boundaries and the reap consumer

### S5 is automated through receipt, not through a proven full journey

`crates/omp-orchestrator/src/main.rs:That` and `src/lib.rs:That` describe an automated observe → queue → finished-pane sweep → dispatch → receiver-receipt path. That corrects the old human-only S5 claim, but it does not prove runtime behavior on the live fleet. The current local `omp-rpc-session` crate is explicitly a transport for **one** `--mode=rpc` child (`crates/omp-rpc-session/src/lib.rs:That`) and does not claim cross-session continuity.

Completion evidence has five separate layers; they must not be collapsed:

| layer | evidence | status |
|---|---|---|
| declaration | upstream `AgentEndEvent` at `dist/types/extensibility/shared-events.d.ts:AgentEndEvent`, with `willContinue` | **AVAILABLE / DECLARED** |
| wire observation | `1408` / `1414`; `.flywheel/inventory-artifacts/agent-end-raw-frame.json.gz` contains {"type":"agent_end","isTerminal":true}; uncompressed SHA-256 `d8bd80c6949b2ec48af1639b5b5e241bd90b4dce1e769483dd1690ed2be8f644` | **WIRE-PROVEN for one terminal frame; artifact bytes are checked by `artifact_provenance`** |
| local parser | `omp-rpc-session/src/lib.rs:AgentEndEvent` recognizes only Ready/Response/Unknown/Malformed | **NOT IMPLEMENTED for AgentEndEvent** |
| local consumer | focused search found no `agent_end`, `willContinue`, `isTerminal`, `RpcSessionEventFrame`, or `AgentEndEvent` consumer | **NOT CONSUMED** |
| reap | `reap-finished-panes` exists and is invoked by `omp-orchestrator` at `main.rs:AgentEndEvent`, but it sweeps finished panes rather than consuming AgentEndEvent | **WIRED for pane sweep; NOT WIRED for completion event** |

`isTerminal: true` is not proven equivalent to `willContinue: false`; one terminal frame cannot
establish non-terminal settle behavior, crashes, killed panes, rate-limited turns, or compaction.
The honest status is therefore **completion AVAILABLE and WIRE-PROVEN, but NOT CONSUMED locally**.
The work moved from inventing a protocol to adopting an existing event plane, but adoption still
requires changing the one-child attachment topology. No completion crate is claimed.

### Reap is a consumer, not an idle observation

The named `reap-finished-panes` crate is **present** and its binary is invoked by the resident supervisor, but it is a finished-pane sweep, not an AgentEndEvent completion consumer. `ack-spine/src/followup.rs:AgentEndEvent` remains a pure candidate classifier, and the focused `classify_followup|followup_action` search found no production caller. It has a measured false-completion path:

* for an open/in-progress bead, unchanged assignee, no comment, and before the deadline, `classify_followup` returns `FollowUpVerdict::VerdictPosted`; and
* `followup_action` maps `VerdictPosted` to `Healthy` at `followup.rs:followup_action`.

That state is **in progress**, not a posted verdict and not a finish. A future consumer MUST
represent it as a distinct non-terminal `InProgress` result. Only a read-back closed row may
produce `Finished`; only `Finished` may authorize refill. `SilentPastDeadline` remains a
follow-up, not a refill. These are **PROJECTED contract repairs**, not claims that the current
candidate has been changed.

The resident cycle (`main.rs:AgentEndEvent`) invokes the finished-pane sweep before reading the ready queue and then proceeds through dispatch/receipt. It has no production AgentEndEvent reap → grade → validation → ship edge. This is the explicit post-dispatch **NO-CLAIM** boundary for the current supervisor.

### Settled wire fact

The `1414` result remains useful and is not withdrawn: `AgentEndEvent` crosses `--mode=rpc` in the captured terminal case. It closes the claim that OMP has no completion precedent, but it does not close the adoption, parser, consumer, or completion-reap claims above.

---

## 11.7 Surface-map counts: measured universe and current WIRE cardinality

R14/R15 batch rows 1–9 contain **270 mapped rows** across `ntm`, `br`, `bv`, and OMP. The
**544-row R14/R15 surface-universe denominator is `DECLARED` from the R14/R15 review** rather
than derived from this section; `NUMBERS.toml` records related surface-map snapshot drift and figure discipline. The named
query below is a historical snapshot: it used 591 rows and SHA-256 f155a358dd302982367a7c0107fe0eb1e3cd6f5ec7d4689bac67f11b1c5063f7. The current map identity is **614 rows, 302,002 bytes, SHA-256 5b3c3238c4ec9dd7f72a097bb3668e7de224e3b6f0eddc1132de2902a1d9d93c**; NUMBERS.toml is the current count authority. — HISTORICAL as of 2026-09-02.

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

HISTORICAL OUTPUT (not current):
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
`loop-queue-filter`, supporting it as the intended S4 graph consumer. The 18 rows pointing to — HISTORICAL as of 2026-09-02.
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
| `S5` Execution | `/ntm`, `/vibing-with-ntm` | robot surfaces and operator doctrine; local completion-event adoption is absent, while finished-pane sweep is wired |
| `S6` Grading the work | `/beads-compliance-and-completion-verification` | prose verdicts; no shared grade value |
| `S7` Validation | `/vibing-with-ntm` | observation and tending; not a production AgentEndEvent completion consumer |
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
upstream `IrcDeliveryReceipt` declaration (`tools/hub/types.d.ts:AsyncJobDeliverySink`) and `AsyncJobDeliverySink`
(`:84`) remain **DECLARED only** and are not evidence that this local S5 path consumes them.

**NO-CLAIM:** this maps the R10 search results; it does not prove that the 16 skills are the only
skills that could participate, or that they compose cleanly merely because they are named here.

---

## 11.9 Stage logging and the S9 decision ledger

Current heartbeat rows written by `write_heartbeat` at `crates/omp-orchestrator/src/main.rs:write_heartbeat` contain `event`, `status`, `tick`, `repo`, `session`, and `detail`. Focused search found no `stage_id`, `from_stage`, or `to_stage`. The old "3 of 9 stages log" statement is withdrawn as a stage-level guarantee: a few files contain records, but those records cannot prove a stage transition.

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

The projected accountable append owner remains `omp-orchestrator`; the human remains the decision owner. The current repository does have `docs/decisions.jsonl` with three manually recorded `HD-<n>` rows, but no automated writer or lifecycle-event artifact was measured. A fail-closed retrieval check for the current manual ledger is:

```sh
set -o errexit -o nounset -o pipefail
test -s docs/decisions.jsonl
jq -e -s 'length > 0 and all(.[]; (.id|type=="string") and (.id|test("^HD-[0-9]{4}$")) and (.binds_stages|type=="array") and all(.binds_stages[]; test("^S[1-9](-[a-z-]+)?$")))' docs/decisions.jsonl >/dev/null
```

This proves nonempty, schema-shaped manual rows only; it does not prove an append owner, stage-event linkage, or amortization. The lifecycle-event writer, session-scoped retrieval, and automatic decision handoff remain **PROJECTED** with owner: S9 implementation lane; next action: create the writer/gate and capture an append/readback transcript.

---

## 11.10 One-to-many namespace and cardinality contract

The current implementation is not a proven 1:many orchestrator. Its source/configuration facts and
behavior are listed below; no captured multi-session runtime probe is presented, so `observed` is
reserved for a captured runtime probe.

| surface | source-derived/static behavior | safe current contract |
|---|---|---|
| resident `omp-orchestrator` process | one configured supervisor process | one process per session until fan-out is proven |
| `omp-rpc-session` | exactly one OMP `--mode=rpc` child; no cross-session continuity | one child per attached session; no cross-session completion claim |
| pane candidates | omp-orchestrator/src/lib.rs:heading_pane_candidates_omp_orchestrator_src_lib_rs counts dispatchable panes and returns dispatchable.first() | one selected pane per cycle; N > 1 must not silently truncate |
| ready beads | omp-orchestrator/src/main.rs:heading_ready_beads_omp_orchestrator_src_main_rs selects bead_ids.first() after parse_ready | one selected bead per cycle; N > 1 must not silently truncate |
| heartbeat ledger | session-named heartbeat path is formed at `main.rs:heading_heartbeat_ledger_session_named_heartbeat_path_is`, while default tick-monitor state and pending-dispatch basenames are formed at `main.rs:heading_heartbeat_ledger_session_named_heartbeat_path_is` | basename reuse can collide across sessions; env overrides exist, but collision refusal is unverified |
| claim permit | one bead ID plus one receiver in `DispatchIntent::Bead` | one bead → one receiver → one permit |
| completion/reap | `reap-finished-panes` is invoked for finished-pane sweep; no local AgentEndEvent consumer | zero automatic completion-based refill claims until a consumer is wired |

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


---

## 11.9 A–Z kernel expansion: target mapping and evidence boundary

Section 12 now owns the operational A–Z map. This section records the evidence needed to keep that
map honest while it is materialized.

### Current versus target

| target | current evidence | target proof still required |
|---|---|---|
| K0 namespace/identity | live pane identity can be read from NTM; Agent Mail identity exists; `binding` can be `legacy-unverified` | register/readback field diff, verified-live binding, cold-start envelope |
| K1 requirements/decisions | manual `HD-*` rows in `docs/decisions.jsonl` | typed append-only writer, retrieval, stage linkage, consumer |
| K2 plan/foundation | plan assembly and freshness prose | foundation artifact, source hashes, materializer caller |
| K3 plan grade | grading rounds and JSONL evidence exist | shared typed grade, held-out lens, identity comparison |
| K4 DAG/selection | `br` and `bv` are installed surfaces | plan-to-beads materializer, cycle/orphan/digest gate, caller |
| K5 claim/lease | `dispatch-claim-fence` authorizes a tracker snapshot; Agent Mail reservations exist | atomic claim/lease lifecycle, expiry, transfer and readback |
| K6 admission/packet | admission, readiness, packet shapes exist in separate crates | one caller and one event-producing kernel path |
| K7 transport | bounded subprocess contracts and NTM send path exist | native adapter parity, transport event hashes, failure receipt |
| K8 receiver observation | receiver receipt contract requires fresh captures | local observation parser and two-capture proof |
| K9 tracker ack | three-authority ack spine is documented | typed `NoAckYet`/`AckRefused`, pending retry preservation |
| K10 completion/reap | one OMP terminal frame is wire-proven; finished-pane sweep is wired | AgentEndEvent parser/consumer, grade-to-reap edge |
| K11 independent grade | fresh-eyes rounds and findings artifacts exist | production grade type and close-evidence caller |
| K12 external validation | stage is named; no transcript is proven | foreign-machine unattended runner and transcript |
| K13 ship/rollback | installer coverage is declared; identity is printed | persisted manifest, rollback transcript, caller |
| K14 memory/skills/closure | archive timeline and inbox surfaces exist; mining is prospective | native AM corpus lane, candidate evaluator, closure event chain |

### Native Agent Mail as a kernel lane

The proposed adapter boundary is deliberately two-ended:

- **Rust core:** typed request/response models, `&Cx` cancellation, field-presence comparison,
  identity/binding policy, event-byte hashing, cursor/read-state reconciliation, and restrictive
  error outcomes.
- **Native adapter:** authenticated Agent Mail daemon/MCP HTTP is primary. The Homebrew `am 0.3.31`
  CLI path is a differential oracle over local storage, not daemon truth. Both boundaries are bounded
  and retain raw request/response hashes.
- **Reconciliation:** daemon and CLI/direct backends are distinct in the event record. If both are
  used, disagreement is `ORACLE_DISAGREEMENT`, never silent preference.

The monitor is not one boolean. `delivery_cursor`, recipient `tail_cursor`, `unread_count/read_ts`,
`ack_required`, and `wake_result` are separate fields. `am inbox-events` supplies delivery position;
`am inbox` supplies read and acknowledgement state; NTM supplies live wake. The documented NTM wait deadline is 300 seconds. A caller ceiling below 300 terminates the observer
first and yields `CANCELED` without cursor information; a 340-second ceiling reaches NTM's `TIMEOUT`
with resumable `cursor_info`. An explicit shorter caller timeout is still REQUIRED for dispatch latency
and cursor preservation. `ntm --robot-attention --attention-cursor=<n>` is the working attention path.
A recipient tail of zero is a no-events state, not a healthy monitor result. Delivery continuity is
recipient-scoped over a global sparse sequence; pair stored cursor, recipient, tail, and
`oldest_available_cursor`, refusing ambiguity rather than claiming eviction. The daemon's documented
`CURSOR_EXPIRED` refusal is currently reported to clamp to a successful page, silently skipping content;
`agent-mail-native` defends this via `journey::verify_resume_continuity` / `journey::resume_from` and
reports the fix upstream.

### Conversation-to-skill mining contract

Mining is a lifecycle lane, not an ad hoc retrospective. For each project namespace it must:

1. capture a bounded timeline/inbox/thread corpus with source and byte hashes;
2. normalize messages without dropping sender, thread, bead, pane, or acknowledgement metadata;
3. cluster repeated operational patterns and count support;
4. attach counterexamples and a falsifier to every candidate;
5. emit a candidate skill with trigger, when-not-to-use, inputs, outputs, failure modes, and refusal;
6. run a held-out replay against an external oracle before promotion;
7. retain rejected candidates and the retry predicate in the negative-evidence ledger.

The 746-event archive supports candidate families but does not yet prove that any candidate is a
materialized skill. In particular, the timeline contains 310 ack-term summaries, 303 of which are
ATC acknowledgement probes; this is evidence for an ack/monitoring candidate, not evidence that
those probes establish liveness or completion. `am inbox-events` returning `inbox_events_unavailable`
for an unregistered/no-pipe/redirected case is correct fail-closed behavior and is excluded from the
silent-success census. Any exit-code audit must isolate the command rather than inspect `$?` after a
pipeline.

### Research boundary

The design uses these original arXiv results only as bounded analogies:

- MetaGPT (`2308.00352`) and ChatDev (`2307.07924`) support structured SOPs and role communication.
- Self-Resource Allocation (`2504.02051`) supports capability-aware allocation hypotheses.
- Magentic-One (`2411.04468`) supports plan/working-memory/replan loop shape.
- AIOS (`2403.16971`) supports an OS-like kernel boundary for scheduling and memory.
- AgentScope (`2402.14034`) supports first-class message/fault/monitor surfaces.
- SAGA (`2605.00528`) supports workflow-level scheduling and affinity hypotheses.
- Reflexion (`2303.11366`) supports retaining feedback as explicit language-level memory.

None proves OMP transport, receiver delivery, comprehension, claim, tracker acknowledgement,
completion, external validation, or shipment. The local contracts and their fired gates remain the
only acceptance authorities.


### JSM skill-library overlay

JSM discovery adds operational depth to this evidence map. The installed library exposes `agent-orchestration` (dependency-aware fan-out/fan-in and completion tracking), `agent-mail` (identity, reservations, threads, ACKs), `agent-monitoring` (layered health/trajectory/SLO), `agent-lifecycle` (version/rollback/retirement), `agent-memory` (raw/summarized/structured memory), `operationalizing-expertise` (provenance and join keys), `self-improving-agent` (reviewed promotion and pruning), `accretive-cron-orchestration` plus `loop-enforcement` (SWEEP/AUDIT/LEARN, tick receipts, escalation), `human-in-the-loop` (risk-proportional approval and demotion), `rust-core-thin-frontend-workspace` (core/harness/thin adapters), `testing-conformance-harnesses` and `oracle-ga…

The JSM status snapshot was online/authenticated with 135 local skills, 85 saved skills, a 47-day-old sync, 47 active bandit arms, 5,473 feedback events, and zero evidence records. Long natural-language searches returned empty sets while exact/one-word searches returned relevant matches; search results require exact-term retries and shape checks. `agent-mail-patterns` appears in remote/private search results but is not installed here, so it is not treated as loaded authority.

| project artifact | required fields | source skills |
|---|---|---|
| skill manifest | project ID, query, skill/version/content hash, trigger, refusal, source message IDs, owner, held-out result | `operationalizing-expertise`, `agent-memory`, `self-improving-agent` |
| dispatch receipt | sender/session/target/bead/detail, input/output hashes, authority, receiver and tracker outcomes | `agent-mail`, `agent-orchestration`, `testing-conformance-harnesses` |
| monitor receipt | recipient, global cursor, recipient tail/oldest, read state, backend, explicit timeout, wake result | `agent-monitoring`, `condition-based-waiting`, `oracle-gates` |
| tick receipt | typed mode, work/plan-space artifact, blocker/escalation, next action | `accretive-cron-orchestration`, `loop-enforcement`, `swarm-operator-loop` |


### Observer and gate hardening corrections

- `omp-orchestrator-calr` is a P0 vacuity finding: an empty staged scan currently returns exit 0. The
  three-valued result must distinguish `CLEAN`, `VIOLATION`, and `NOTHING_TO_CHECK`; an empty index is
  not clean.
- Pipe exit status is not command exit status. Any exit-code audit MUST isolate the command and capture
  its status directly; `$?` after a pipeline is inadmissible evidence.
- `path-literal-guard` and `state-wildcard-lint` have inconsistent treatment of inline test code. This
  is a gate-asymmetry finding requiring either a runtime-constructed fixture or a named gate fix; the
  fixture itself is not silently promoted as proof.
- Agent Mail `inbox_events_unavailable` is correct fail-closed behavior and is excluded from the
  silent-success census. The daemon `CURSOR_EXPIRED` clamp remains a source defect defended by
  `agent-mail-native` continuity checks and reported upstream.

### Scope gate

K0–K14 is a logical architecture map. It must be staged before bead creation. The recommended first
implementation wave is K0/K5/K6/K7/K8/K9 because it closes the dispatch/ack/comms spine; K2/K3/K4/
K10/K11/K12/K13 follows for plan-to-ship; K1/K14 closes decisions, mining, learning, and run closure.
Joshua approved this staging on 2026-09-02; the scope decision is recorded before bead creation.

**NO-CLAIM.** This section names the native AM lane, the exact event-byte contract, the mining
workflow, and the research boundary. It does not claim that any K0–K14 lane is complete or that a
current event writer, monitor, completion consumer, or skill evaluator exists.

### 11.10 Native AM wiring boundary

`crates/agent-mail-native/` now contains the typed daemon client, journey operations, cursor newtypes,
wake wrapper, CLI differential oracle, and ignored live journey tests. A scoped search of
`crates/omp-orchestrator/` found no reference to `agent_mail_native`, `MailClient`,
`fetch_inbox_events`, or `wait_for_mail`. The native lane therefore exists and has local contract
tests, but has no production caller in the resident supervisor. This is the concrete BUILT ≠ WIRED
boundary for K0/K7/K9/K14; it is not a completion claim.

The first implementation bead must wire one real caller through the typed core, then prove the daemon
path, the CLI differential oracle, receiver/tracker separation, and the explicit timeout/cursor rules
at that caller. An isolated green live-test crate does not close the lifecycle edge.

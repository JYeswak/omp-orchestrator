# The spine walk — one TOML row per box, cited, exhaustive branches

Joshua, 2026-09-03: "the entire spine for every path of this process needs to be rooted out before we
build more." This directory is the machine artifact of that walk: `boxes/<Sx>.toml`, one file per box,
one owner per file (path-scoped commits, no collisions), integrated into `flow.toml` and rendered by
`frankenmermaid` (installed: `~/.local/bin/frankenmermaid`, `fm-cli 0.2.0`). `lifecycle-arrow-gate`
(bead `x226`) reads `flow.toml`; a box without a crate is MISSING, a crate without a reachable trigger
is UNWIRED, and both fail the build.

## Row shape (exactly this; unknown = the literal string MISSING, never blank, never guessed)

```toml
[[box]]
id = "S5a"                       # S1 S2 S3 S4 S5a S5b S6a S6b S6c S7 S8 S9
name = "select: ready ∩ unclaimed ∩ non-epic, bv-ranked, claim ATTEMPTED"
trigger_today = "hand"           # hand | hook:<name> | launchd:<unit> | cron | slash:/<skill> | none
trigger_target = "launchd:omp-orchestrator run"   # where it MUST fire once wired
crate = "loop-queue-filter"      # a dir under crates/, or MISSING
crate_status = "exists-no-caller"   # wired | exists-no-caller | MISSING   (caller = main.rs, hook, or a crate dep — cite it)
bead = "omp-orchestrator-2ceb"   # or "no bead"
kernel_input = "BeadSnapshot (crates/omp-orchestrator/src/dispatch_packet.rs:L?)"   # Rust type + path:line, or PROPOSED <Type{fields}>
kernel_output = "ClaimAttempt | ClaimRefused{blockers} (PROPOSED)"                  # the Verdict-shaped type
event_row = "LifecycleEvent{stage_from=S4, stage_to=S5a, actor, pane, incarnation, outcome, reason_code=CLAIM_REFUSED, blocker={chain}}"
validator = "bead-lint (2lqd) before claim; attempt-claim is the readiness test"
sota_standard = "PageRank over the dependency DAG; work the articulation points"
sota_cite = "mirror:beads_rust/<path>:L? | this-repo:<path>:L?"   # every cite VERIFIED by re-running sed -n / ls. fh is NOT a citation authority.
skill = "/beads-bv, /beads-north-star"
branches = [
  "epic -> REFUSED accounting node (05.10)",
  "claim refused -> next rank; journal row names blocker chain",
  "claim ok -> S5b",
]                                # EXHAUSTIVE: every enum arm of the crate's verdict type, verbatim names
no_claim = "what this box does not establish"
```

## Citation rule (the whole point)

- `this-repo:<path>:L<n>` — you ran `sed -n '<n>p' <path>` and the line says what you claim.
- `mirror:<repo>/<path>` — under `/Volumes/ZestData/dicklesworthstone-mirror/`, and `ls` confirms it. A
  path under OUR `crates/` is never a mirror cite (measured tonight: a scout labelled `ack-spine` and
  `no-shell-gate` as `mirror:beads_rust/...` — fabricated; refused).
- `fh:<row>` is **NO LONGER A VALID CITATION**. Ruled out by Joshua 2026-09-03: chronically stale
  (`SEARCH_INDEX_STALE`, `digest_missing_today`, and an independent `fh doctor` returning
  `DAILY_SCHEDULE_CARDINALITY_DRIFT` — expected one 05:15 invocation, found 0), and its `fh-ledger`
  row was the SOLE red gate in the admission chain, so a stale index was blocking every dispatch.
  Cite source directly. A row whose only support is an `fh` id is unsupported and is refused.
- `branches` = the enum arms of the crate's real verdict/action types (`AckAction`, `ReceiptReason`,
  `PaneState`, `Liveness`, `ClosePrefix`, `DispatchAdmissibility`, ...) verbatim, plus the human-halt arm.

## Process

1. Bring your current bead to `STAGE: IMPL -> GRADING` or release it. No new feature bead until this lands.
2. Write your boxes. Commit path-scoped: `git commit -- docs/plan/flow/boxes/<Sx>.toml`.
3. Post one bead comment on `omp-orchestrator-x226`: `SPINE: <boxes> landed <sha>; disagreements: <list>`.
4. Cross-review round follows (you read another pane's boxes; a disagreement is a finding, not an edit).

## Wave 1 addendum (Joshua, 2026-09-03 05:3xZ): "not ready — name everything it's missing, physics for every aspect, Rust hooks, all agents agree after healthy debate, as long as it takes; stop building"

**BUILD FREEZE.** No new crate, no new feature bead, no install, until every box has `agreement.status = "converged"` AND Joshua's approval row in `docs/decisions.jsonl`. The four in-flight beads (`d3gm`, `6nhj`, `ywd5`, `gfb`) go to GRADING or release; nothing new is claimed.

Every box file gains four sections. A box without all four is not reviewable.

```toml
# 1. PHYSICS — every number in the row, with the command that produced it and when. A number
#    without a command is a guess dressed as a fact (planning-workflow: "Grounding").
[[box.measurement]]
claim = "preregistration-gate is first in the pre-commit chain"
command = "sed -n '121,122p' crates/no-shell-gate/src/bin/pre-commit-gate.rs"
value = "L121 validate_staged_preregistration; L122 refusals.push(\"preregistration-gate: …\")"
measured_at = "2026-09-03T05:20Z"
by = "AmberGate"

# 2. GAPS — everything the box is missing, each with the thing that resolves it. The nine crate-atom
#    parts (d3gm) and the seven rigor layers L1–L7 (AGENTS.md) are the checklist: a box that does
#    not name its claim row, SLO row, oracle, fuzz target, wired-caller test, event row, and diagram
#    has gaps whether or not it lists them.
[[box.gap]]
what = "no LifecycleEvent row is written for S2 (S2 logs nothing)"
resolves = "kxe.8 journal writer + vcd7.1 emit site at plan-assemble's write chokepoint"
class = "event_row"        # event_row | claim_row(L1) | slo(L2) | oracle(L3) | gate_trip(L4) | fuzz(L5) | formal(L6) | lock(L7) | wired_caller(atom 9) | verdict_type(atom 3) | diagram | hook | measurement

# 3. AGREEMENT — the debate ledger. A box converges when a wave yields ZERO open disagreements from
#    all three non-owners (steady state, planning-workflow validation loop #4). Waves repeat as long
#    as it takes. A disagreement is a row, never an edit to another pane's box.
[box.agreement]
wave = 1
owner = "AmberGate"
reviewers = ["BlueLantern", "SilverWolf", "QuietRidge"]
open_disagreements = 0
status = "draft"            # draft | reviewed | converged | approved (approved = Joshua's HD row id)
approval = ""               # "HD-00NN" when Joshua approves

# 4. DIAGRAM — the box's own if/then flowchart, one node per branch, generated from `branches`
#    (hand-written until x226 emits it), validated: `frankenmermaid validate diagrams/<Sx>.mmd
#    --fail-on warning` exit 0, rendered: `frankenmermaid render diagrams/<Sx>.mmd --format svg`.
[box.diagram]
mmd = "docs/plan/flow/diagrams/S2.mmd"
validate_exit = 0
```

**Hooks are Rust, certified, or they are not hooks.** Reference: `~/Developer/control-plane/hooks_certified.toml`
(16 rows; schema `id, event, matcher, class advisory|gate, fail_mode, binary, policy_file,
source_commit, language, stage, certified, harm_class`) and the substrate crate
`control-plane/crates/zestgraph-hook-substrates`; contract = stdin JSON `{tool_name, tool_input}` →
stdout `{"hookSpecificOutput":{"hookEventName","permissionDecision"}}` (probed live on
`zestgraph-danger-gate`: benign command → `allow`). Every `[[box.hook]]` row names its future
`hooks_certified.toml` row and its gauntlet stage (`/hook-certification`, six stages). No `.sh` hook, ever.

**Disagreement rows** go in `docs/plan/flow/waves/wave-<N>/<reviewer>-<Sx>.toml`:
```toml
[[disagreement]]
box = "S2"
field = "crate_status"
owner_says = "wired"
reviewer_says = "wired to the hook only; no supervisor caller — call it hook-wired"
evidence = "grep -rn plan_assemble crates/omp-orchestrator/src -> 0"
severity = "major"         # blocker | major | minor
resolution = ""            # filled by the owner: accepted | refuted(with evidence) | escalated(HD-00NN)
```
The owner answers every row in the next wave. `refuted` needs evidence the reviewer can re-run;
`escalated` means Joshua decides. A wave closes when every row has a resolution.

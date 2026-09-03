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
sota_cite = "mirror:beads_rust/<path>:L? | this-repo:<path>:L? | fh:<row-id>"   # every cite VERIFIED: sed -n / ls / fh why
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
- `fh:<row>` — only if `fh why <row>` resolves. **fh is STALE right now** (`SEARCH_INDEX_STALE`,
  `digest_missing_today`, index lock held by the fh swarm); say so in the row and cite source instead.
- `branches` = the enum arms of the crate's real verdict/action types (`AckAction`, `ReceiptReason`,
  `PaneState`, `Liveness`, `ClosePrefix`, `DispatchAdmissibility`, ...) verbatim, plus the human-halt arm.

## Process

1. Bring your current bead to `STAGE: IMPL -> GRADING` or release it. No new feature bead until this lands.
2. Write your boxes. Commit path-scoped: `git commit -- docs/plan/flow/boxes/<Sx>.toml`.
3. Post one bead comment on `omp-orchestrator-x226`: `SPINE: <boxes> landed <sha>; disagreements: <list>`.
4. Cross-review round follows (you read another pane's boxes; a disagreement is a finding, not an edit).

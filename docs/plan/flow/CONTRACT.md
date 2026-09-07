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

> ⛔ **SUPERSEDED IN PART — DO NOT STOP READING HERE.** The freeze above is **LIFTED FOR S1** by the
> Wave-2 amendment below, which reads "S1 IS AUTHORIZED TO BUILD. S2–S9 REMAIN FROZEN." S2–S9
> remain frozen exactly as written. **Do not conclude you may not build until you have run:**
>
> ```bash
> # Anchored on the bold heading form so the pointer you are reading is NOT a hit: exactly 1.
> grep -nE '^\*\*S1 IS AUTHORIZED TO BUILD' docs/plan/flow/CONTRACT.md
> ```
>
> **LINE NUMBERS REMOVED 2026-09-07, and the reason is the more useful half.** This pointer said
> `:101`, was corrected to `:116` after `grep` showed `:101` landed on the strangulation-cycle
> diagram — and **the correction invalidated itself in the same edit**: adding eight lines above the
> target pushed the text to `:124`, then to `:132`. Three wrong numbers, each wrong the moment it
> was written.
>
> **A line-number cross-reference inside a living document is SELF-INVALIDATING.** Not a careless
> mistake — a structural property: any edit above the target shifts it, and the edit most likely to
> be made is the one fixing the pointer. **A pointer with a wrong target is worse than no pointer**,
> because it converts "I did not look" into "I looked and it was not there." Cite a searchable
> string and let the reader's `grep` resolve the address. Same family as this repo's rule to search
> for a *fragment* of a name rather than the whole name.
>
> **The anchor is `^\*\*` deliberately.** The unanchored form returns **3** hits, two of which are
> this pointer quoting the phrase — the self-referential-instrument defect this repo has now hit
> eight times (a census whose table names every gate it checks; a doc comment containing the needle
> it warned about; a `jq` probe asking whether a list contained itself). **An instrument whose input
> contains text about its own subject reports on itself.** Anchored: exactly 1 hit, the amendment.
>
> This pointer exists because the amendment **downstream in this file** was invisible for a full
> session (the prose here said "42 lines below"; it is now 79 — the same self-invalidating defect,
> in words instead of a number, so the distance is deliberately no longer stated):
> the orchestrator read "BUILD FREEZE" in bold, stopped — which is the correct reading of a
> document that says STOP — told the fleet all code was blocked, and routed three panes to audits
> while **131 authorized S1 beads sat claimable**. A superseding amendment placed downstream of the
> text it supersedes cannot be found by a reader who obeys that text.
>
> **Note the gate ids in the authorization prose below are MISTYPED:** they read
> `gate-s1-10-jtgw … -15-w44h`; the real ids are `gate-s1-l0-jtgw … gate-s1-l5-w44h` — `l0`
> (ell-zero) written as `10` (one-zero). Searching for the contract's spelling returns ABSENT for
> all six and makes the authorized set look empty. **Never copy them from this file.** The correct
> six live in `AGENTS.md`; this runner was executed before being published and returns exactly 6:
>
> ```bash
> grep -oE 'gate-s1-l[0-9]-[a-z0-9]+' AGENTS.md | sort -u    # -> 6 ids
> ```
>
> **The first version of this note shipped a runner against THIS file that returns 0**, because the
> correct ids appear in `CONTRACT.md` only inside the warning you are reading — so the check would
> have "proved" the authorized set empty, which is the exact failure the note exists to prevent.
> **A published runner that was never executed is an unverified claim wearing a command's
> authority.** Anti-vacuity leg: `gate-s9-l[0-9]` returns 0, so the 6 is a real match, not a
> pattern that matches anything.

## Wave 2 amendment — THE FREEZE'S EXIT CONDITION IS UNREACHABLE (measured 2026-09-03)

**The freeze above cannot be satisfied, and the measurement proves it rather than arguing it.**

Pane 4, on bead `omp-orchestrator-denominator-invisible-growth-7t33`:

```
DECLARED_AT_HEAD=211  COVERED_AT_HEAD=43
GROWTH_PER_COMMIT=19.22  CLOSURE_PER_COMMIT=0  CONVERGES=no:growth>>closure
trajectory: 6238c0f 38 -> 8de39a5 62 -> 933432d 204 -> fe30a57 211 -> e6dac73 211
```

**`CLOSURE_PER_COMMIT=0` is the freeze's arithmetic, not a discipline failure.** S1's requirements
are overwhelmingly *artifacts*: a `LifecycleEvent` writer per layer, artifact+readback, a monitor,
a gate with a known-bad leg, a metric, a named test — 60 of the coverage rows name a test. Every one
of those needs code. The freeze forbids code. So the only permitted activity is auditing, auditing
*discovers* requirements, and `MISSING` rises monotonically while the single operation that could
lower it is prohibited.

**And the exit condition closes a cycle.** The freeze lifts on `agreement.status = "converged"`.
S1 was flipped to `converged` at `a9ea672` and Joshua **retracted** it — *"s1 is not converged; you
can't claim converged when s1 is still missing massive functionality."* So convergence now requires
the functionality to exist; the functionality requires a crate; the crate requires the freeze
lifted; the freeze lifts on convergence.

```
converged  ->  needs functionality  ->  needs a crate  ->  needs freeze lifted  ->  needs converged
```

That is exactly the strangulation `AGENTS.md` records one level down — an epic holding a `blocks`
edge onto the leaf it owns, which killed 13 of the first 30 unassigned beads including four P0s.
**Same shape, applied to the process instead of the graph.** A cycle reads as "not ready yet"
forever, which is why five agents could work all night and converge nothing.

### The amendment (Joshua, 2026-09-03, verbatim)

> *"i think we need to plan out and build s1 fully and prove it works across all aspects of the
> lifecycle before we move to s2 — build this in waves"*
> *"keep s0 / s1 planning going and lets get it executed when ready"*

**S1 IS AUTHORIZED TO BUILD. S2–S9 REMAIN FROZEN.** Scoped, because the original freeze was right
about everything except its own reachability:

- **Permitted:** crates, feature beads, tests, and installs whose bead is wired to an S1 layer gate
  (`gate-s1-10-jtgw`, `-11-fnv8`, `-12-j5m9`, `-13-z8hz`, `-14-hs15`, `-15-w44h`) or to
  `gate-s1-djn8`. Each lands behind its own gate with a known-bad leg, per the "every gate proves it
  bites" rules in `AGENTS.md`.
- **Still frozen:** any crate, bead, or install for S2–S9, and any canonical mapping change to
  boxes S2–S9. `gate-s1-djn8` still blocks `gate-s2-ehx8`, so S2 cannot start until S1 closes.
- **Unchanged:** `approval` is still an HD row id. The build proceeding does not grant approval; it
  removes the reason approval can never be earned.

**This amendment is dated and written here because it was a DISPATCH-ONLY INSTRUCTION for hours.**
Joshua said it in-session; pane 1 recorded it at `S1.toml:220` as a note and never wrote it into the
contract, so every pane kept reading the unamended freeze and kept filing findings instead of
building. That is the precise failure `AGENTS.md` names: *a dispatch-only instruction is an
unrecorded requirement*, and the requirement it lost was the authorization to make progress.

**NO-CLAIM.** Lifting the freeze for S1 does not make S1 converge. It makes `CLOSURE_PER_COMMIT`
capable of being non-zero, which is a precondition for convergence and not a substitute for it. The
readiness number itself is still not regenerable from a clean clone
(`MATRIX_FROM_HEAD=no:untracked-generator`), so until that is fixed no closure figure is
reproducible by anyone but this working copy.

## Wave 3 amendment — ATLAS ARC GOVERNS, AND NO AUTO-REFILL UNTIL THE PROCESS IS PROVEN

Two rulings from Joshua on 2026-09-03, in order. The first replaces the sequencing above; the
second constrains the dispatch lane the product exists to automate.

### 1. Atlas Arc supersedes the hand-written sequencing (HD-0014)

> *"yeah use that new proces i just pasted"* … *"that was mined from jeff's plans — lets follow it
> more closely than my hand written work."*

The second sentence is operative: it ranks the mined Dicklesworthstone process **above Joshua's own
hand-written instructions**, which includes the Wave-2 authorization to build S1 first. **R1,
breadth before depth, is now binding:**

```
max(section_maturity) - median(section_maturity) <= 1

MEASURED 2026-09-03 — VIOLATED at 2:
S1      meas=6  gap=10  contracts=1  diagram=2  beadrefs=273  MATURITY=4
S2..S9  meas=5-8 gap=9-19 contracts=0 diagram=1 beadrefs=0-3  MATURITY=2
```

R1_POPULATION_BOXES=S1,S2,S3,S4,S5a,S5b,S6a,S6b,S6c,S7,S8,S9

The R1 denominator is that named set (HD-0014's twelve stage boxes), not
"whatever is on disk" and not the 13 numbered `docs/plan/[0-9][0-9]-*.md`
files. `r1-breadth-gate` refuses a box on disk omitted from this pin, and
refuses if this row is deleted. Numbered sections are scored in the same
run; they do not enter the max/median/delta.



**No `CRITICAL_PATH_EXCEPTION` is recorded.** One was recommended; the ruling makes it unnecessary,
because the order it would have excepted is itself superseded. The gate now in force: *no further S1
depth until all twelve section records carry purpose, inputs, outputs, dependencies, consumers,
unknowns, non-goals and source pointers.* Two S1 builds are **suspended, not cancelled** — the L0
install suite and `crates/s1-coverage`; landed work is retained.

**A partial refutation is recorded with the adoption.** Atlas Arc asserts later sections *"remain
empty"*. Measured, they do not: each carries 5–8 measurements, 9–19 gaps, an agreement block and a
diagram. The accurate statement is narrower — **S1 alone has a contracts block and bead ownership.**
The *mechanism* diagnosis stands on our own prior numbers rather than the document's authority:
`GROWTH_PER_COMMIT=19.22` against `CLOSURE_PER_COMMIT=0`, and a freeze whose exit condition closed a
cycle.

**Known blocker on the mechanical half:** only three `.md` files were provided. `scripts/arc.py` was
not, so `init`, `lint`, `status` and `certify-plan` **cannot run** and `BUILD_READY` is **UNRUN**,
not failed — per Atlas Arc's own rule an unavailable tool is UNKNOWN, never a pass. `arc.py` is also
Python, which the one rule forbids as a tracked file; it lives in the skill directory outside this
repo, as the readme-update validator does.

### 2. No auto-refill (binding)

> *"we dont want to automatically refill panes with work without being approved — we have to reap
> every update and get docs updated — we dont want auto refill until our process is proven."*

**This is not less automation. Notification is wanted; actuation is not.** A pane finishing must
reach the conductor as a loud event — Joshua has been serving as that event all session, which is
the defect being fixed. What the conductor does next is a judgement:

```
pane finishes → LOUD notification → conductor REAPS → docs updated → approval → dispatch
```

Every arrow is a stop, not a pass-through. `refill-idle-panes --plan` may **propose**; it may not
send. A plan is a proposal for a human, never an authorization, and the selector must carry an
explicit `APPROVAL_REQUIRED` marker so a downstream actuator cannot mistake one for the other.
`inbox-monitor` (mail → loud blocker) is therefore fully in scope; the refill lane stops at a plan.

**Measured state of every auto-dispatch path into this repo, 2026-09-03:**

- **cron** — 50 uncommented rows; **zero** dispatch into `omp-orchestrator`. The two matching a
  dispatch-shaped pattern are `cfs-honesty-tick` and control-plane's `tick-ledger-identity-check`,
  both other projects. Every `refill` / `controller-tick` / `loop-driver` / `fast-dispatch` row named
  in `AGENTS.md`'s post-mortem is **commented out**, so that section is STALE for this repo.
- **launchd** — 18 plists exist; exactly **one** is loaded:
  `ai.zeststream.omp-orchestrator.control-plane`, running
  `omp-orchestrator --repo …/control-plane --session control-plane`, `RunAtLoad=True KeepAlive=True`.
  It points at **control-plane, not here.** `ai.zeststream.omp-orchestrator.plist` exists on disk and
  is loaded **zero** times.

So "can anything auto-refill this session right now" is **NO**, measured rather than assumed. The
constraint is satisfied by **absence, which is not enforcement** — nothing prevents loading the plist
or uncommenting a row.

**Why this sequencing is right and not timidity.** The product's claim is that an idle worker beside
a ready queue is the conductor's failure. But measured tonight in this repo: a selector that returns
`UNMEASURABLE` and cannot select, a `fleet-monitor --self` that resolves the wrong session, an
uninstalled `inbox-monitor`, three blocking waits that each failed for a different reason, and a
conductor that published five instrument-produced readings. Automating dispatch on top of that would
industrialise the errors.

**NO-CLAIM.** This records a policy and a measurement; it installs no gate. Nothing refuses a
dispatch that lacks recorded approval, so the constraint is honoured by convention — the enforcement
class this repo distrusts most.


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

# 3. AGREEMENT — the debate ledger.
#
#    RETIRED DEFINITION (Joshua, 2026-09-03): "a box converges when a wave yields ZERO open
#    disagreements from all three non-owners". That rule is now REFUSED. Agents drift into
#    agreement: "if you told me the sky was brown and I said ok, it's brown, settled, then the
#    result is a wrong premise. CONVERGED WRONGLY IS WORSE THAN NO CONVERGENCE."
#
#    MEASURED THE SAME DAY, which is why the rule changed. Across all 88 wave-1 disagreement rows
#    in every box, reviewer rejections = 0 — not one row opened with a rejection. Wave 1 was a
#    SURVEY, not a review. And owner refutations = 0 everywhere: the S1 owner answered ten rows and
#    accepted ten, then flipped the box to converged. Four reviewers on S1 in wave 1 produced fewer
#    challenges than one reviewer in wave 2 (WildMountain 9 rejections / 12 rows). Reviewer COUNT is
#    not a quality signal; a wide net recruits agreers.
#
#    THE BAR NOW HAS FOUR CLAUSES. All four, or the box stays draft.
#    a. ZERO OPEN ROWS IS NECESSARY, NOT SUFFICIENT, and alone is evidence of nothing.
#    b. A wave MUST produce at least one REFUTATION THAT CHANGED THE BOX — a row whose resolution
#       begins `refuted(` and carries the evidence, filed by someone who is not the owner. A wave of
#       all-accepted rows is a FAILED WAVE. Rerun it with a different reviewer.
#    c. THE OWNER MUST REFUTE, TOO. An owner whose accept rate is 100% is not answering, they are
#       agreeing. Refute at least one row per wave, or state per surviving row why it survived.
#    d. A reviewer row MUST carry a FALSIFIER ATTEMPT, not an opinion: the command that could have
#       proven the box right and did not. Measured exemplar — pane 4 graded the owner's L4OMP claim
#       by enumerating four live omp PIDs and reporting LISTEN_ROWS=0, unix_named=0. That beat the
#       owner's topology inference and turned it into a measurement. An "I agree" row is worth zero.
#
#    AND THE FUNCTIONAL FLOOR: no box may reach `converged` while any of its layers reads
#    `exists = "none"`. Agreement about a MAP is not working software. Every layer needs a wired
#    caller, an event row with a WRITER, a monitor, a gate with a known-bad leg, and a proof run.
#
#    Every wave prints three numbers or it has not been reviewed: rows, rejections, refutations.
#    A disagreement is a row, never an edit to another pane's box.
[box.agreement]
wave = 1
owner = "AmberGate"
reviewers = ["BlueLantern", "SilverWolf", "QuietRidge"]
open_disagreements = 0
rows = 0                    # DERIVED per wave: grep -c '^\[\[disagreement\]\]'
rejections = 0              # reviewer rows opening REJECT/REFUTED. 0 across a whole wave = failed wave
refutations = 0             # resolutions beginning refuted( . Clause (b) requires >= 1 from a non-owner
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

## R3 seventh class — UNBUILT (bead omp-orchestrator-1qzt)

Atlas Arc R3 ships six dispositions: `ASSUME_REVERSIBLY`, `BLOCKS_PLAN`, `DEFER_TO_BEAD`,
`OUT_OF_SCOPE`, `HUMAN_DECISION`, `BLOCKS_BUILD`. Those six are insufficient for this
spine. `docs/plan/flow/unknowns/CENSUS.json` measured 177 `[[box.gap]]` rows; **150** have
a known closer (named bead, `MISSING`+wire, add-fuzz, register-claim). Forcing
`BLOCKS_PLAN` or `HUMAN_DECISION` onto them manufactures uncertainty and idles panes on
questions that are not questions.

**UNBUILT** is the seventh class. It is known work, not an unknown. Same shape as
`DECLARED-UNEXTRACTABLE` and `UNFALSIFIABLE-RUNNER`: fewer buckets than the data.
The typing function is `docs/plan/flow/unknowns/R3-disposition.mmd` (`UNBUILT seventh`
on the "named bead / MISSING+wire" arm). Do **not** open an HD row whose question is
"should we add a fuzz/SLO/claim row".

Census command (re-run, do not copy):

```
python3 -c 'import json; print(json.load(open("docs/plan/flow/unknowns/CENSUS.json"))["UNBUILT"])'
```

NO-CLAIM: this records the vocabulary. It does not close `02ai` and does not rewrite
`boxes/*.toml`.


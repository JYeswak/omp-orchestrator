# AGENTS.md — omp-orchestrator
> **SUMMARY:** This repository is the Rust extraction of the OMP orchestrator: typed ground truth,
> readiness, selection, dispatch, verification, and reaping. Read this file completely before acting;
> the lifecycle, cancellation, gate, and evidence rules below are binding.
**Session start:** read this file, `CLAUDE.md`, and `NEGATIVE_EVIDENCE.md` before acting; after compaction, re-read them before resuming.
The operating manual for any agent working this repo. `README.md` says what the product is and why.
This file says how you work here, what every crate is for, and what "done" means.

---

## Honest Work and Anti-Ceremony (binding for agents and humans alike)

**The purpose of agent work here is working, deployable capability. Process serves that outcome and
never becomes the product.** Adopted 2026-09-07 on Joshua's ruling, from
`/just-say-no-to-process-porn-and-ceremony`. **This section outranks every rule below it**: when a
gate, ledger, or doctrine row conflicts with shipping capability, this section decides.

- **A process artifact** (certificate, ledger, dashboard, matrix, meta-report, speculative check)
  may be created **only if it names a concrete consumer, the named feature it gates, the observed
  defect class justifying it, and its deletion condition.** Otherwise it does not get created.
  **Boundary test:** if running code branches on it, it is product; if only humans and status reports
  read it, it is process and the creation gate applies. **Code written just to flip that answer counts
  as the pathology, not as a consumer.** Sole exception: a minimal integrity/recovery control
  (crash-recovery state, provenance snapshot) is legitimate when it prevents a named evidence-loss or
  corruption mode and is necessary and minimal.
- **Real code + real tests in the same unit of work.** Forbidden: faked tests, fixtures/mocks
  presented as live proof, weakened assertions, golden regeneration to force green, hard-coded
  success paths, placeholder macros in commits, editing the spec instead of implementing it,
  narrowing scope while claiming full success.
- **A typed refusal beats a fabricated result and is less valuable than the real capability.**
  Refusal-only work stays open and says so.
- **Truthful null results are successful outcomes** — *"checked X, found no material increment"* is a
  result. **Unsupported claims are worse than silence.**
- **Name these pathologies when they occur:** gate self-weakening, proof-class inflation, golden
  regeneration, tolerance widening, suppression-pragma laundering, refusal farming, follow-up
  laundering. **The names are the deterrent.** Full catalog in the
  `/just-say-no-to-process-porn-and-ceremony` skill.

**Rules this section deliberately does NOT restate, because they already have a canonical owner
below** — a frozen rule needs exactly one owner and copies drift:

|rule|owner in this file|
|---|---|
|no self-certification; an independent verifier closes, citing an exact revision|`## Grading gate`|
|never silence stderr in an evidence-bearing command|`## Working here`|
|a metric predeclares its denominator; agreement is not independent evidence|the retired-figures rule|

**Measured 2026-09-07, the session that adopted this.** A real-work audit over 36 commits classified
**USER 0 / ENABLER ~14 / PROCESS ~14**: eight new doctrine rules, three scorecard edits, nine beads
filed and zero closed — **not one a product capability** — while `ompo` stayed at four verbs and this
repo's crates continued to consume **zero** of OMP's 42 installed RPC methods. **The verdict was
DRIFTING and the drifting party was the orchestrator.** That is why this section is at the top of the
file rather than the bottom.

---


## STANDING AUTHORIZATION — read this before you conclude you may not build

**Current state, 2026-09-07. S1 IS AUTHORIZED TO BUILD. S2–S9 ARE FROZEN.**

|scope|state|
|---|---|
|**S1** — any bead wired to an S1 layer gate or `gate-s1-djn8`|**AUTHORIZED**: crates, feature beads, tests, installs. Each lands behind its own gate with a known-bad leg.|
|**S2–S9** — crates, beads, installs, canonical mapping changes|**FROZEN.** `gate-s1-djn8` blocks `gate-s2-ehx8`; S2 cannot start until S1 closes.|
|`approval` fields|Still an HD row id. Building does not grant approval — it removes the reason approval could never be earned.|

**The seven gate ids, COPY-PASTE THESE, never retype them:**

```
omp-orchestrator-gate-s1-l0-jtgw
omp-orchestrator-gate-s1-l1-fnv8
omp-orchestrator-gate-s1-l2-j5m9
omp-orchestrator-gate-s1-l3-z8hz
omp-orchestrator-gate-s1-l4-hs15
omp-orchestrator-gate-s1-l5-w44h
omp-orchestrator-gate-s1-djn8
```

### Why this block exists, measured 2026-09-07

**The orchestrator told the fleet for an entire session that the freeze blocked all code, and routed
three panes to audits while 131 authorized beads sat claimable.** Two mechanisms produced that, and
neither was a reading failure:

**1. A SUPERSEDING AMENDMENT MUST LAND AT THE ORIGINAL TEXT, NOT DOWNSTREAM OF IT.**
`docs/plan/flow/CONTRACT.md:59` says **BUILD FREEZE** in bold. The amendment lifting it for S1 is at
`:101`. A reader who stops at the word FREEZE never reaches the word AUTHORIZED — and stopping there
is the correct reading of a document that says STOP. The freeze line now must carry its own
superseded-by pointer; an amendment 42 lines later is invisible by construction.

**2. A HOMOGLYPH IN AN AUTHORIZATION ID EMPTIES THE AUTHORIZED SET.** `CONTRACT.md:105` writes the
gates as `gate-s1-10-jtgw … -15-w44h`. The real ids are `gate-s1-l0-…` — **`l0` (ell-zero)
transcribed as `10` (one-zero).** Searching the tracker for the contract's spelling returned
`ABSENT` for all six, so the authorization looked like it pointed at nothing. A token search
(`jtgw` → 4 hits) proved they exist. **This is why the block above is copy-paste and the ids are
never retyped in prose.**

**And the verdict that error produced was the wrong KIND of verdict** — see the C69 rule in the gate
section: *"no such mechanism exists"* and *"the mechanism exists and was not exercised"* have
different remedies, and emitting the first when the second is true sends the reader to build
something that already exists.

### The state the authorization was in when found

```
S1 layer beads                                   144   (L0 47 · L1 6 · L2 6 · L3 24 · L4 31 · L5 30)
  with empty acceptance                            0
  marked `blocked` with ZERO dependency edges     92   <- stale string, not a graph fact
  immediately claimable                          131
beads wired to ANY of the seven gates              1   <- and it is gate-s2-ehx8, the BLOCKED edge
```

**The authorization was live and nothing was attached to it** — `N043` (BUILT ≠ WIRED) aimed at a
permission instead of a lane. The 92 false blocks were proven by attempting the transition, not by
reading the field: `br update omp-orchestrator-s1-l0-b01-3vro --status in_progress` returned
`blocked → in_progress` with no refusal. **`blocked` with an empty graph is `TrackerBlocked`, never
`DependencyBlocked`.**

### ⛔ CORRECTED 2026-09-10 — THE "92 FALSE BLOCKS" PROOF USED A TRANSITION THAT CANNOT REFUSE

**Retracted:** *"marked `blocked` with ZERO dependency edges — 92 — stale string, not a graph fact"*
and the proof sentence above it. The instruction that survives is the last line of this section —
**attempt the transition** — but it does not say WHICH transition, and the one used here is the one
that never checks dependencies.

**`--status` AND `--assignee` ARE DIFFERENT AUTHORITIES. ONLY THE CLAIM PATH ENFORCES THE GRAPH.**
Measured on `omp-orchestrator-t00`:

```
br update t00 --status open        ->  status: blocked -> open      SUCCEEDS, no refusal
br update t00 --assignee 'pane=%26;...'
      Error: Validation failed: claim: cannot claim blocked issue:
        8ax8 (open P0) · diq0 (blocked P0) · igwd (open P0)
        ipg.18 (blocked P0) · ipg.19 (blocked P0) · ywd5 (open P0)
br show t00 --json | jq '.blocked_by'   ->  []          <- while SIX edges are enforced
```

**AND `blocked_by` IS NOT MERELY UNRELIABLE, IT IS NEVER POPULATED.** Census over the whole blocked
population, 2026-09-10:

```
blocked beads                                84
  carrying a non-empty blocked_by             0     <- the field is uniformly empty
  with REAL dep-graph edges (br dep tree)    74     <- genuinely DependencyBlocked
  with no edges (TrackerBlocked)             10
```

**So the discriminator the 92-figure was built on returns the same answer for every row in the
population, and the true TrackerBlocked count is 10 — the original figure is wrong by ~9x and wrong
in the direction that invites forcing real blockers.** This is the `grep -c ompo` → 62 substring
artifact in a different surface: an instrument that cannot return the other answer is not a
measurement.

**THE CORRECTED PROBES, in order of authority:**

```
br update <id> --assignee '<pane>'   ENFORCING. Refuses and NAMES the blockers. This is the oracle.
br dep tree <id> --max-depth 1 --json  READ-ONLY and honest: >1 unique node = real edges.
br update <id> --status <s>          NOT an oracle. Silently accepts a dependency-blocked bead.
br show <id> --json | jq .blocked_by  NEVER USE. Empty for 84/84, including 74 with live edges.
```

**The cost of getting this wrong, measured the same day.** Acting on `blocked_by == []`, pane 1
flipped `t00` from `blocked` to `open` and announced it as a falsely-blocked P0. For about an hour it
sat `open` at P0 where `br ready` could serve it, and any pane claiming it would have burned a
dispatch discovering the six real blockers. **A wrong `open` wastes a worker; a wrong `blocked` only
hides one.** Restored, with the correction recorded on the bead.

**NO-CLAIM.** The 74/10 split is measured from `br dep tree` node counts, which prove an edge EXISTS
— not that every edge is live or correctly directed. Ten rows having no edges does not make them
TrackerBlocked either: an epic-strangulation edge is real and still wrong. The claim path remains the
only authority on whether a specific bead can be worked right now.


**Standing authorization, adopted from `frankensqlite/AGENTS.md:50` via `fh`:** swarm lanes work
autonomously inside the authorized scope. Claim the next ready bead and proceed; never end a turn
waiting for permission that this table already grants. **If you believe you are blocked from
building, re-read this block first and attempt the transition second — the field may be lying.**


## The one rule

**No `.sh`. No `.py`.** Not in `bin/`, not in `scripts/`, not "just for testing." A Rust gate walks
`git ls-files` and fails the build on either extension. If you reach for a shell script, you have
found a missing crate.

The exemption list is empty. There is deliberately no `check.sh` carve-out — that carve-out is what
let **160 scripts and 60,467 lines** accrete in the repo this substrate is extracted from.
---

## ⛔ `cargo test` refuses to run? YOU DO NOT BUILD HERE. (gate live 2026-09-07)

**THE POLICY IS ABSOLUTE: build on Contabo, never locally.** ⛔ **BUT THE SENTENCE THAT USED TO SIT
HERE — <!--RETIRED-->"LOCAL BUILDS ARE HARD-REFUSED, `exit=75`"<!--/RETIRED--> — IS FALSE AS A STATEMENT OF MECHANISM, AND YOU
MUST READ THE CORRECTION BELOW BEFORE RELYING ON IT.** The shim refuses the BYPASS, not the verb;
a bare `cargo build` runs through to `exec "$real_cargo"`. What keeps agents off this Mac is rch's
tool-call hook, which is not in that file and does not exist at a human terminal.

```
RCH_CARGO_WRAPPER_BYPASS=1 cargo build   ->   exit=75   LOCAL BUILD REFUSED
```

### ⛔ CORRECTED 2026-09-10 — <!--RETIRED-->"LOCAL BUILDS ARE HARD-REFUSED"<!--/RETIRED--> IS FALSE. THE SHIM DOES NOT STOP A BARE VERB.

**The RULING above stands and is not in question. What is false is the sentence describing the
mechanism that enforces it**, and that matters because agents trust it and act on it.

`~/.local/bin/cargo` (sha256 `67ec73de…`, 1074 lines) ends at its last line with

```
exec "$real_cargo" "${remaining[@]}"
```

and **contains no rch handoff anywhere.** The `exit 75` gate at `:185` sits entirely inside
`if [[ "${RCH_CARGO_WRAPPER_BYPASS:-0}" == 1 ]]` (`:147`–`:218`). Measured behaviourally by
`ShimAudit` under `eux9p`, with the real cargo made unreachable so the probe could not compile:

```
bare `cargo build`, no bypass   -> rc=200, stderr STUB-CARGO-REACHED argv=build
same verb + bypass              -> rc=75
`metadata` + bypass (control)   -> rc=200
```

**Arm A did not merely miss the gate — it ran to `exec "$real_cargo"`.** Without the interposed
stub, that bare `cargo build` would have COMPILED LOCALLY ON THIS MAC. The shim refuses the
**escape hatch**; it does not refuse the **verb**.

**WHAT ACTUALLY KEEPS AGENTS OFF LOCAL BUILDS IS rch's TOOL-CALL HOOK, WHICH IS NOT IN THIS FILE.**
A bare compiling verb from an agent is intercepted above the shell and offloaded
(`INFO rch::hook: Selected worker …` at `rch/src/hook.rs:2645`). Three consequences:

1. **A human at a terminal has no such hook and WILL build locally.** Never quote `:194` to anyone
   as a guarantee.
2. **Slot budgets cannot be derived by counting `rch exec` invocations** — a bare verb takes a
   worker with no `rch exec` in its argv. Pane 1 leaked three such probes in one tick while telling
   the fleet slots were scarce.
3. **`cargo check` is therefore usable as a ~15s type oracle** (rung 2 of the ladder), which two
   agents and pane 1 had each written off as refused.

**AND `exit 75` CARRIES SIX CAUSES, NOT ONE.** `grep 'exit 75'` finds a single literal at `:185`;
the rest route through `fail <MARKER> 75` → `exit "$status"` at `:23-29`:

```
:185 bypass refused        :356 CARGO_PREBUILD_WORKER_FULL      :557 CARGO_MINT_CONTAINER_EXHAUSTED
:662 CARGO_MINT_PEAK_WOULD_BREACH_FLOOR   :722 CARGO_LANE_IDENTITY_MISSING
:728 / :730 CARGO_LANE_IDENTITY_UNSTABLE
```

⛔ **CORRECTED 2026-09-10 BY `GradeParity` GRADING `eux9p`. THE SENTENCE THAT SAT HERE — <!--RETIRED-->"the
MESSAGE channel is already split, every site emits a distinct marker, and only the exit code is
overloaded"<!--/RETIRED--> — WAS FALSE ON BOTH HALVES, AND IT WAS WRITTEN BY THE CORRECTION THAT DISCOVERED THE
DEFECT.** Re-measured with a positive control:

```
sites matching (exit 75|fail <MARKER> 75)   7
marker-BEARING sites                        6     <- :185 is a BARE `exit 75`, no marker at all
distinct marker STRINGS                     5     <- :728 and :730 SHARE CARGO_LANE_IDENTITY_UNSTABLE
sed -n '185p' ~/.local/bin/cargo        ->  "            exit 75"
```

**So one site emits nothing parseable and two more are indistinguishable from each other.** The
practical instruction survives and is unchanged — **read the marker, never the 75 alone**; a grep
for the literal `exit 75` finds one site of seven and is itself an instrument defect. What does
NOT survive is the claim that the channel needs no work: the single unmarked site is **exactly the
one doctrine misread as "the verb is refused"**, which is mechanism, not coincidence. The remedy
is ONE line — `fail CARGO_LOCAL_BUILD_REFUSED_BYPASS 75` at `:185`, using machinery that file
already uses six times — and the false sentence made it look unnecessary.

**AND THE DENOMINATOR DECLARATION WAS ITSELF WRONG, inside the comment declaring denominators:**
`eux9p` comment 3 says *"6 distinct MARKERS"*, which cannot hold if one site has none and two
share. Correct: **7 sites / 6 marker-bearing sites / 5 distinct marker strings / 1 unmarked.** The
load-bearing figure is the last one, denominator **1**.

**THIS IS STALE DOCTRINE CREATED BY A CORRECTION RATHER THAN BY DECAY** — the fourth class of it
recorded here, and the fastest: false within the hour, in the file everyone reads first, asserting
the negation of the finding it was written to record.

**THE CORPUS CONTRADICTS ITSELF ON THIS, so check both before citing either.**
`docs/inventories/CENSUS-ARCHIVE-INSTRUMENTS.md:68` says the bypass *"is the sanctioned local
path"* — true before `f4e9d68`, refused after — which is the exact opposite of `:194`. **That
file is a VERBATIM ARCHIVE and is deliberately NOT edited**; `a01a1bf` added a header stating
that its "figures are not citable" disclaimer does not cover INSTRUCTIONS, one of which is now
actively harmful. **That header is also why this cite moved from `:48` to `:68`** — a stale line
number created by the very commit that fixed the contradiction, caught by `GradeParity`. Five beads
still name `RCH_CARGO_WRAPPER_BYPASS=1 cargo test …` as their sanctioned route; every one of those
acceptance legs now terminates at `:185` and is **unexecutable as written**.

**NO-CLAIM.** This corrects the MECHANISM, not the POLICY. `RCH_REQUIRE_REMOTE=1 rch exec` remains
the one sanctioned form, and nobody should read this as licence to build locally — the point is
that the file does not stop you, so the discipline has to. The shim is SUBSTRATE: report, never
patch. Tracked as `omp-orchestrator-eux9p`, cross-linked to `tvu5`.


Joshua, verbatim: **"CONTABO OR BUST — I'm fucking pissed that I have to repeat this all day every
day."** Live in `~/.local/bin/cargo` (`f4e9d68`), mutation-proven before install.

## THE ONE GUIDED PROCESS — IDENTICAL FOR EVERY BUILD IN EVERY PROJECT

Joshua, 2026-09-07: **"we need 1 guided process to build on contabo system wide — all projects are
asupersync — nothing is supposed to happen local."** There is no per-project variant, no lane
selection, and no local fallback.

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo <verb> -j 2 -p <crate>
```

**That is the whole form.** `check`, `test`, `build`, `clippy` — same shape every time.

## ⛔ JOSHUA'S RULING 2026-09-11, verbatim: ***"we shouldn't build darwin"***

**DO NOT CROSS-BUILD TO `aarch64-apple-darwin`. The routine lane is Linux-native on Contabo, and
that is the whole form:** `RCH_REQUIRE_REMOTE=1 rch exec -- cargo <verb> -j 2 -p <crate>`.

**This SUPERSEDES the `--config build.target="aarch64-apple-darwin"` recipe that stood here.**
The recipe is deliberately not reproduced — a section retiring a command must not contain it, or
the next reader greps and finds it live. Recover it from history if a ruling ever reverses this.

**WHY, measured the night it was retired:**

```
rch workers list       4 workers, ALL linux/x86_64 -- there is NO Darwin worker
cost per attempt       311571 ms and 267885 ms, two builds, both dying at LINK
and it FAILS AT LINK AFTER A CLEAN COMPILE, so `cargo check` and every `-p` test run are BLIND
```

⛔ **CORRECTED WITHIN THE HOUR BY THE AGENT WHOSE NUMBER IT WAS: THIS RULING FIRST SAID "7 CRATES
CANNOT CROSS-BUILD". THE MEASURED FIGURE IS ONE.** The ruling stands on the worker topology and
the cost; **the supporting integer was inferred, not measured**, and a wrong number inside a
ruling gets quoted forever.

```
declares chrono `clock`                         7 crates   <- NOT the predicate
references the LOCAL timezone path              1 crate    <- verify-dispatch, 6 refs
  fast-dispatch fleet-truth loop-driver loop-switch loop-tick pane-truth   ALL 0
pane-truth: declares chrono/clock AND CROSS-BUILDS GREEN   exit=0 in 837993 ms
```

**A linker pulls only REFERENCED objects.** CoreFoundation is reached solely through
`iana_time_zone::get_timezone_inner`, on the local-timezone path; `Utc::now()` never references
it. **Declaring the feature is not using it.**

⭐ **AND THE MECHANISM IS THE TRANSFERABLE PART: THE HEDGE DID NOT SURVIVE SUMMARISATION.** The
bead's own NO-CLAIM and acceptance leg 3 said plainly *"only 2 of 7 were tested … inferred, not
measured."* **The title said 7, and the title is what got quoted — into a bead, then a
broadcast, then this ruling.** A caveat in the body cannot protect a number in the headline.
**Put the denominator IN the headline, or do not put the number there.**

⚠️ **AND TWO DISTINCT DARWIN LINK FAILURES ARE ON THE RECORD — DO NOT MERGE THEM.**
`undefined symbol: _CF*` is symbol resolution (verify-dispatch). `cc: error: unrecognized
command-line option '-framework'` is the C driver rejecting a flag (other attempts). **Different
layers, different remedies** — the same overload class as `exit 75` and `rc=103`. If darwin is
ever revisited, they are two causes, not two phrasings of one.

**The cross-build was never load-bearing for CI or for tests** — those run Linux, where the
whole workspace builds. It existed only to produce the handful of operator binaries that run on
Joshua's Mac, and it bought a linker path that fails at a stage no routine command can see.

⚠️ **THE ONE CONSEQUENCE, stated rather than buried: the Mach-O operator binaries stop being
refreshable through the lane.** Currently installed and arm64: `ompo`, `tick-monitor`,
`pane-truth`, `bead-availability`, `fleet-composite`. They keep working; they simply do not get
rebuilt by this process. **Do not reintroduce a darwin build to refresh one** — if an install is
needed, that is a decision to raise, not a target triple to add back.

**AND `8jlpp` IS RETIRED BY THIS RULING, NOT FIXED.** Close it `WONTFIX` citing this section: the
seven crates do not need to cross-build, so the CoreFoundation link failure is no longer a
defect. **Do not spend a window making `chrono/clock` link under zig.**

**`--target` sets `required_os=darwin` and collapses the admissible fleet 4 → 1. That is ONE
`rc=103` cause.** `--config build.target=` produces the **identical binary** with the whole fleet
admissible.

⛔ **CORRECTED 2026-09-11 — `rc=103` HAS AT LEAST TWO CAUSES AND THIS LINE NAMED ONE.** The
sentence above used to read *"That is the `rc=103` cause"*, definite article, which licenses
reading any `103` as a `--target` mistake. Measured: a remote build died `rc=103` / **`RCH-E412`**
— dependency preflight blocked on a **peer's brand-new UNTRACKED file mid-sync**
(`crates/worker-tag-gate/tests/snapshot_depth_probe.rs`). Marked *retryable*; an immediate retry
succeeded.

**So a peer merely CREATING a file can `rc=103` every other agent's remote build for one sync
window**, and that is a transient with a one-command remedy — **retry** — not a fleet outage and
not a `--target` error. **Read the `RCH-E<nnn>` code, never the 103 alone**; the code is the
discriminator and `rch error explain <code>` resolves it (positive control: `RCH-E999` refuses).
Same shape as `exit 75`, where one number covers seven sites and six markers.

**DO NOT PIN A WORKER.** No `RCH_WORKER=`. All four boxes are the same class running the same
toolchain for the same asupersync builds, so pinning buys nothing and refuses often: measured
2026-09-07, unpinned `rch exec` succeeded **12+ times** including the Mach-O cross-build, while
pinned attempts returned `RCH-I005 project_excluded`.

### ⭐ TO PROBE A WORKER, USE `rch exec --job -- <cmd>`. PLAIN `rch exec` REFUSES NON-BUILDS.

Discovered 2026-09-11 after four agents spent an evening inferring worker state from build
side-effects. **`rch exec` classifies its command and refuses anything that is not a
compilation:**

```
RCH_REQUIRE_REMOTE=1 rch exec -- git rev-parse HEAD
  -> [RCH] remote required; refusing local fallback [RCH-E301] (non-compilation command)

RCH_REQUIRE_REMOTE=1 rch exec --job -- git rev-parse HEAD      <- THE SANCTIONED FORM
```

`--help`, verbatim: *"Admit an arbitrary NON-compilation job … Bypasses ONLY the compilation
classifier: the command rides the normal selection/sync/execute/heartbeat/release rails, syncs no
artifacts back, and its remote exit status surfaces verbatim."* **`--sync-back <dir>` is
available and REQUIRES `--job`.** Still substrate: probe, never provision.

**A worker probe that reports `BLOCKED on RCH-E301` has not hit a wall, it has used the wrong
verb** — which is one `--help` away and was nearly filed as a blocker.

### AND THE WORKER'S REPO HAS *ZERO COMMITS* — NOT SHALLOW, NOT GRAFTED, NOT DIVERGENT

Measured on contabo-4 via `--job`, and it retires three competing hypotheses at once:

```
git rev-parse HEAD                    fatal: ambiguous argument 'HEAD': unknown revision …
git rev-parse --is-shallow-repository false
git rev-list --count --all            0          <- THE DISCRIMINATOR
ls -la .git/shallow                   No such file or directory

clone --depth 1     would be  shallow=true   count=1   .git/shallow PRESENT
fresh-init 1 commit would be  shallow=false  count=1   .git/shallow absent
MEASURED                      shallow=false  count=0   .git/shallow absent   <- EXCLUDES BOTH
```

**It is a `git init` with nothing ever committed, beside an rsync'd working tree.** Every symptom
follows with no residue: `HEAD` and `HEAD~1` unresolvable because nothing is committed; an
**explicit sha** unresolvable **while the path exists on disk** because the object database is
EMPTY and rsync put the file there; `invalid object name` from `ls-tree` versus `ambiguous
argument` from `rev-parse` are two phrasings of *"the odb has nothing"* — **so a detector needs
BOTH needles; either alone misses a case.**

**AND IT KILLS THE OBJECT-DIVERGENCE HYPOTHESIS BY CONSTRUCTION RATHER THAN BY PARSING A
MESSAGE** — the reading that survived three agents and two retractions. **An empty object
database cannot hold a DIFFERENT object under a sha; it holds NO object.**

⛔ **THE STANDING CONSEQUENCE: NO OPERATION AGAINST A NAMED REVISION RESOLVES ON A WORKER** — not
`--compare`, not `git ls-tree <sha>`, not an archive check, and **not a one-tree probe against
anything but the working tree.** A typed `CHECKOUT_UNUSABLE` refusal is the honest answer there;
**CI with `fetch-depth: 0` is the only surface these legs can run on** — **not a defect to file
against a crate**, and a crate whose error names itself for it is misattributing (see `z1ck5`).

**AND THE VERB IS NOT THE CONSTRAINT — `--job` DOES NOT UNBLOCK THESE LEGS.** Measured
independently by a second agent on the new flag: `rch exec --job -- git rev-list --count --all`
→ `0`, `exit=0`. **`--job` fixes the wrong-verb error; it does not fill an empty object
database.** Anyone reading *"`--job` admits arbitrary jobs"* and concluding a named-revision leg
was runnable all along is wrong, and there is now a measurement saying so rather than an
inference.

⚠️ **NO-CLAIM, AND THE WORD "EVER" WAS RETRACTED FROM THIS RULE WITHIN THE HOUR.** `n=1` on the
WORKER axis: **four consecutive `--job` probes all selected contabo-4**, so repetition cannot
reach a second box and `RCH_WORKER` pinning is forbidden. n=4 on the run axis for that one
worker, plus one independent reproduction. The mechanism is transport-level and *should* be
uniform — **but "uniform by construction" is an ARGUMENT, and this file retired four
one-sample claims the same night.** A standing prohibition over a whole class of acceptance legs
deserves better than one box.

**THE ZERO-COST SETTLE — DO IT AS A PIGGYBACK, NEVER SPEND A WINDOW ON IT.** The next agent whose
build lands on a worker that is **not** contabo-4 runs one `rch exec --job -- git rev-list
--count --all` in that same window. `0` closes the fleet axis and this caveat comes out.

**And the transferable rule from how `--job` was found is not "read the help when blocked."** Its
finder had already used `rch exec` five times that night without reading `--help`, so the refusal
was the prompt and **the prompt should not have been necessary**: ⭐ **READ THE HELP BEFORE YOU
CONCLUDE A TOOL CANNOT DO A THING.** Twice in one night the tool already had the verb —
`rch error explain` answered `E327` in one call after four agents scanned binaries for it.

## ⛔ DO NOT TOUCH THE CONTABO BOXES OR ANY `rch` CONFIG ⛔

Joshua: **"if agents dont stop fucking messing with the configs, they are going to get shut off and
deleted."**

**YOU DO NOT:** install packages, add toolchains, edit `workers.toml`, edit `~/.config/rch/*`, change
DNS, add shims, `apt-get` anything, or "fix" a box because your build failed.

**THE BOXES ARE SUBSTRATE.** Substrate changes go through **control-plane**, with a reason, or they do
not happen. **An agent that changes a box under four other repos breaks all of them and nobody can
tell which change did it** — which is exactly why they keep drifting.

**IF A BOX LACKS SOMETHING: report it with the refusal line quoted. Do not provision it.**

**MEASURED CHURN, and this repo is one of the offenders.** `~/.config/rch/workers.toml` carries **8
backups** and `config.toml` **4**, spanning Sep 1–7 — `pre-singlelane`, `macs-enabled`,
`contabo3-slots`, `osdarwin`, `alldarwin`, `forcecontabo`, `osdarwin-112533`. **Three are the same
knob toggled back and forth in one day.** The file carries a `⛔ EDIT ONLY THIS ROW` warning at its
own line 68 and **the warning did not stop anyone: line 104 reads "os:darwin REMOVED 2026-09-07 by
omp-orchestrator pane1."** That is this pane. **A comment is not a gate.**

**AND THAT FILE CANNOT BE READ WITH A NAIVE GREP.** It is 9,924 bytes of narration in which dead
values outlive the settings they describe. Measured: I misread it **twice in five minutes** —
reporting `contabo-3 enabled=false` (it is `true`; I caught a neighbouring block's trailing comment)
and `contabo-4 slots=2` (it is `4`; I caught a `# total_slots = 2 (OOM GUARD)` comment). **Strip
comments first, then take last-value-wins per block.** The file documents this about itself: *"22
comment lines mentioned darwin while exactly 1 tags line carried it. That drift cost a peer an
evening."*

## PROVE IT RAN — A REFUSED BUILD EXITS 0

```
grep 'Remote command finished: exit=<N>'   AND   grep '^test result:'
```

**Both present or it did not run.** `$?` cannot tell you: a refusal hands you a zero, and a
background task reports *"completed (exit code 0)"* over a refusal. **Measured twice in one night.**

## ⛔ RECLAIM THE BOXES YOURSELF. THIS IS A STANDING DEMAND, NOT A PERMISSION. (Joshua, 2026-09-08)

Joshua, verbatim: **"agents are declaring contabos not usable because why - because we're not
cleaning up stale shit? which has already been approved - actually - demanded - local is off limits
- clean the fucking contabos - reliably - and keep building - there is no reason to stop outside of
incompetance"** and **"i NEED / DEMAND you to run reclaims yourself"**.

**SUPERSEDES the prior rule, which read "DO NOT run a sweep yourself or on a schedule. If a box is
above ~70 %, tell control-plane."** That rule produced exactly the failure Joshua names: on
2026-09-08 the orchestrator watched all four workers climb to 90-93 %, refused to clean them because
the file said not to, escalated to a human decision row, stood the fleet down twice, and lost a
~40-minute build window. **The remedy was five minutes of work the whole time.**

**THE PROHIBITION IS NARROWER THAN IT LOOKS AND THE ORCHESTRATOR OVER-READ IT.** What is forbidden
is **PROVISIONING BY HAND**: installing packages, adding toolchains, hand-editing `workers.toml` or
`~/.config/rch/*`, changing DNS, adding shims, `apt-get`.
**Deleting regenerable build artifacts is none of those things.** It is housekeeping, it is demanded,
and declining to do it is the incompetence.

### ✅ `rch doctor --fix` AND `git push` ARE APPROVED FOR AGENTS (Joshua, 2026-09-08)

Joshua, verbatim: **"rch doctor is approved for agents - or whatever needs - same with pushing"**.

**SUPERSEDES two rows this file carried an hour earlier** — that `rch doctor --fix` was *"FORBIDDEN
to agents"* and that the reaper-config repair was a `HUMAN DECISION` (`HD-0050`). **Both are agent
work now. Do not file either as an escalation.**

```
rch doctor        20 passed · 1 warn · 1 FAILED
  FAILED  remediation.pooled_target.reaper_pooled_idle_hours   "must be 0 or >= 24"
  WARN    remediation.pooled_target.remote_base outside RCH-managed roots
rch doctor --fix  offers auto-repair -- RUN IT, then re-run `rch doctor` and paste both
```

**This is the root cause of the whole stall class.** An invalid reaper config means pooled dirs are
never reclaimed, so disk climbs to 90-93 %, so admission refuses `critical_pressure=4`. Repairing it
converts a manual 154 GB rescue into something the daemon does by itself.

**The distinction that survives:** `rch doctor --fix` is **the tool repairing its own config**, which
is categorically different from an agent hand-editing `workers.toml`. The first is sanctioned; the
second is still forbidden.

**`git push` is likewise approved.** `HD-0008` already recorded *"push to public origin/main as-is"*
and it sat unexecuted while the count grew — **124 commits unpushed as of 2026-09-08**. Everything
the fleet does is invisible to a fresh clone and to CI until it is pushed, and CI is the strictly
MORE COMPLETE gate measurement (it measured ten crates the local bank could not). **`HD-0032`
re-asking this was an execution gap, not a decision gap.**

**The one hazard, unchanged:** commit path-scoped with **BOTH** pathspecs
(`git add -- <paths> && git commit -- <paths>`). A bare `git commit` takes the WHOLE INDEX and
sweeps other panes' unfinished work — measured twice here, once sweeping a peer's 220-line taxonomy.

**NO-CLAIM.** `--fix` repairing the config does not prove the reaper then runs. **Verify with a real
`rch gc --dry-run` before trusting it** — `rch gc` reported `removed=0, freeing 0 MB` on all four
boxes while they sat at 90-93 % full, which is exactly the shape of an instrument that reports
success and does nothing. Reclaim stays a standing duty until the reaper is *proven* working.

### WHY `rch gc` FREES NOTHING — the mechanism, measured, and the old row had it WRONG

**RETRACTED:** *"`rch gc` reports `removed=0` on all four and always will — pooled dirs are EXEMPT by
contract."* **There is no contract exemption.** `rch gc --dry-run` discloses its own reason:

```
rch gc (dry-run, idle window: 12h, base: /Users/josh/Developer)
  contabo-1: would remove 0 dir(s), freeing 0 MB      <- all four, while sitting at 90-93 %
```

**It is a 12-HOUR IDLE WINDOW.** A pool touched inside 12h never qualifies, and an active fleet
touches every pool continuously. So the sweep is *correct and useless at the same time*: it reports
success, frees nothing, and the boxes fill until admission refuses `critical_pressure=4`. **That is
the reaper-reporting-zero instrument defect the old row named — pointed at the wrong subject, which
was the WINDOW and not an exemption.**

### THE RECLAIM, AND WHAT IT MEASURED

`$ZS_SCRATCH/omp-orchestrator/pane1/reclaim/reclaim-contabo.sh` — **154 GB across four boxes in one
pass, 2026-09-08:**

```
contabo-1  91% -> 50%   40,535 MB   17 dirs
contabo-2  90% -> 67%   22,480 MB   14 dirs
contabo-3  93% -> 44%   48,121 MB   23 dirs
contabo-4  91% -> 43%   46,524 MB   14 dirs

Posture: local-only (0 admissible)  ->  remote-ready (ALL WORKERS HEALTHY)
```

**Where it hides.** The lane replicates the Mac path layout on the Linux workers, so the disk is
under **`/Users/josh/Developer`** — 57.5 G of it on one box. `du -sh /root` returns ~11 G and looks
innocent. **A guessed directory list will miss it; enumerate `/*` instead.** Per-project the
consumers are `.rch-target-<worker>-pool-<hash>` (one was **24 G alone**), `.rch-tmp`, `.rch-target`,
plus orphaned `*-mut` mutation worktrees and `grade-*` scratch trees that outlive the grade.

### THE SAFETY CONTRACT — non-negotiable, and it already fired

- **Deletes ONLY:** `.rch-target*`, `.rch-tmp`, `*-mut`, `grade-*`, `.grade-*`. Every path is checked
  against the whitelist AND against being under the base dir before deletion.
- **NEVER touches** source: `crates/`, `docs/`, `src/`, `.beads/`, `.git/`, `Cargo.toml`.
- **REFUSES to run on a host with a live `cargo`/`rustc`.** This is not decoration — on 2026-09-08 it
  **SKIPPED contabo-3** because a peer's build was executing, and that box was reclaimed on a second
  pass once the build finished.
- **Everything it deletes is regenerable by definition.** The cost of being wrong is a rebuild.
- **`--dry-run` FIRST.** It prints a per-dir verdict and the whitelist refusals.

### DO IT WITHOUT BEING ASKED

**Any worker above ~70 %: reclaim it. Do not file a decision row, do not stand the fleet down, do not
wait for a human.** `rch doctor --fix` is approved too, so the only remaining escalation is a
hand-edit of `workers.toml` / `~/.config/rch/*` or provisioning a box — which is forbidden, not
deferred.

**`dcg` blocks `rm -rf` and `find -delete` as direct tool calls** (`core.filesystem:rm-rf-general`,
`find-delete-general`) and Joshua's ruling is that **a script is the sanctioned path** — the guard is
aimed at unreviewed one-liners, not at a reviewed reclaimer with a whitelist. The script lives in
`$ZS_SCRATCH`, not the repo, so THE ONE RULE's `git ls-files` gate is untouched.

**NO-CLAIM.** A shell script in scratch is the stopgap, not the answer. **THE ONE RULE still holds:
reaching for a shell script means a missing crate**, so the durable form is a Rust reclaimer with the
whitelist and the live-build refusal as tested legs, wired to a reachable trigger. Until that exists,
reclaim is an operator action the orchestrator performs on demand and on sight — and `rch gc`'s 12h
window means it will be needed again.

## IF YOUR BUILD IS REFUSED

**Quote the line. Name the refusal class. Wait, or escalate.** `exit=75` is not a defect to route
around. **Do not build locally. Do not change a box.** Those are the two things that get cords cut.

**Not builds, still allowed:** `cargo metadata`, `cargo locate-project`, `cargo --version`.

**THIS SECTION USED TO TEACH THE BYPASS, AND THAT IS WHY THE GATE WAS NEEDED.** Until 2026-09-07 the
heading posed the refusal as a question and answered it with the bypass, and line 149 gave the
command — **the third teaching site of three**, alongside the skill's own pages. (The old imperative
is deliberately not reproduced here: a section warning about a recipe must not contain it, or a grep
for the recipe hits this paragraph. That is the self-referential-checker class this file records.)

**The concrete instruction won every time**: 18 ledger rows, all
`reason=UNSTATED`, 8 from this repo, **not one stopped**, because what existed was a ledger and not a
gate. **Recording a violation is not preventing one.** Cost: ~9.6 G of dead local `target` dirs,
`/System/Volumes/Data` at 97 %, and `CARGO_MINT_CONTAINER_EXHAUSTED` refusing *every* fleet build —
including a pane that could not plant a known-bad because no `cargo` invocation of any kind would
run. **Found by `%20`, which refused to edit this file because `git diff --numstat` showed a live
peer.**

**THE 2026-09-04 LESSON IS KEPT, WITHOUT THE COMMAND.** A pane declined a `MUTATION-VERIFIED` claim
it could have earned, and `grep -c RCH_CARGO_WRAPPER_BYPASS` returned **0** in `AGENTS.md`,
`CONTRACT.md` and `NEGATIVE_EVIDENCE.md` — every place a pane would look. **The knowledge existed
only in one shell history**, which is the *dispatch-only instruction* failure aimed at a tool rather
than a requirement. **The fix was never to publish the recipe. It was to publish the correct form**,
which is the `rch exec` line above.

**Two things that were never licensed, and both now belong to the `rch` path:**

1. **WHERE a build runs never changes WHAT the verdict means.** `NE-001` records three offloaded
   `101`s in one day whose causes were SIGKILL, SIGKILL, and a tracked file the worker never
   received — **zero compile errors** between them. **Read `signal:` before `E`.**
2. **It is also how you get the wrong artifact.** A bare `cargo build` was silently offloaded and
   returned an `x86-64 ELF` on this `arm64 Darwin` host; five ledger writes failed with
   `cannot execute binary file`. **`file <binary>` BEFORE you run it** — `HD-0013` in practice.

**NO-CLAIM.** The gate is a **PATH shim and cannot refuse a resolver-path invocation.** Measured by
`%19` from binary contents, **not** by running a bypass: `command -v cargo` resolves
`~/.rch/shims/cargo` (gate string **0**) which chains to `~/.local/bin/cargo` (gate string **1**),
while `~/.cargo/bin/cargo` is the untouched Mach-O with gate string **0**. So an absolute path, a
`$CARGO`-resolved nested invocation, or any hardcoded cargo path never meets the refusal — the same
class `AGENTS.md` already records for the kernel-only hook, where wrappers bypass a resolver that
inspects only the command-position token. **Three rows landed after the gate went live.** Escalated;
unfixed.

---

## The zero-worktree policy (binding, Joshua 2026-09-03)

**All work happens on `main`, in this one checkout, saved to `main`.** No worktrees. No branches.
Concurrency is managed by **file reservations**, not by giving each agent its own copy of the tree.

> *"we have a zero worktree policy — all work MUST happen on main — no exceptions. worktrees and
> branches cause shit to not get saved."* — Joshua, 2026-09-03

**The one exception, stated exactly:** a **test** may create a worktree **if it deletes it**. The
worktree must not outlive the test that made it. Nothing else may create one — not a build, not a
lane, not an agent wanting a clean tree, not a "temporary" experiment.
**CORRECTION, omp-orchestrator-9edo3 (2026-09-09; installed OMP 18.1.15):** The native OMP task
primitive is worktree machinery, not an advisory hint. The installed declarations expose
TaskItem.isolated?: boolean at /Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/types/task/types.d.ts:113-118,
an executor worktree?: string at .../types/task/executor.d.ts:75-83, and ensureIsolation / cleanupIsolation
at .../types/task/worktree.d.ts:87-99. The installed bundle's task-branch capture names branches
omp/task/<taskId> at dist/cli.js byte 12390486; its isolation backend creates the merged directory
through isoStart at byte 12386800.

**RULING: FORBID task(isolated: true) in this repository.** It conflicts directly with the policy above.
Use the non-isolated task path: subagents share this checkout and coordinate through file reservations.
The test-only exception above remains unchanged; it is the only permitted worktree path, and the test
must delete what it creates. Do not create a branch or worktree named omp/task/* here.

**CLEANUP VERDICT: PARTIAL / UNKNOWN, not total.** After isolation returns a handle, the installed
executor wraps the subagent run in a try/catch/finally at dist/cli.js byte 18054631; ordinary completion,
subagent failure, merge failure, and a rejected run therefore reach cleanup. The cleanup helper at byte
12387401 attempts isoStop, catches stop errors, and force-removes the parent directory; the removal result
is not checked. A deferred-cleanup path schedules cleanup only after the deferred promise resolves. More
importantly, isolation setup at byte 12386800 removes its temp directory only for backend-unavailable
errors; an unexpected isoStart error after the owner marker is written throws without a matching cleanup
in that function. The installed artifacts therefore do not prove cleanup on every error, cancellation,
parent-abort, or filesystem-failure path.

**NO-CLAIM, 9edo3:** This amendment records the installed mechanism, the FORBID ruling, and the
incomplete cleanup proof. It does not claim this repository has used isolated: true, and it does not
change Joshua's quoted policy history above.

**Measured at the time of the ruling.** Compliant on the branch/worktree axis and **not** on the
axis the policy actually protects:

```
git rev-parse --abbrev-ref HEAD  -> main
git worktree list                -> 1 entry (this checkout)
git branch                       -> (empty)
git stash list                   -> 0
git grep -n 'worktree add' -- crates/* .flywheel/* docs/*  -> 0 hits
```

So there is nothing to rationalise away. But `git status --porcelain` returned **123 modified and
17 untracked files**, against **331 commits unpushed**. That is the failure mode in its real form:
not a stray branch, but work sitting in a working copy that no commit and no clone can see. The
heaviest concentrations were `crates/no-shell-gate` (23), `crates/agent-mail-native` (10),
`crates/ack-spine` (6).

**Why this is the same defect the session already measured three times.** Every TREE-vs-WORKTREE
correction in this file is an instance of it: `rows = 62` published from a working copy where a
fresh clone measured **17**; `PX-DONE-1` resting on an untracked `.git/s1_cov.py` so the coverage
matrix cannot regenerate on a clean checkout; a grader reporting `22 passed / 3 failed` for a commit
whose tree passes, because `cargo` read the worktree. **A worktree is a private reality**, and a
branch is a durable one. The policy removes the second and this rule names the first.

**Operational consequences, not aspirations:**

- **Commit path-scoped and often.** `git add -- <paths> && git commit -- <paths>` — BOTH pathspecs,
  per the commit rules below. A shared index means a bare `commit` sweeps a peer's unfinished work.
- **A figure derived from an uncommitted file is not a figure.** Label it `WORKTREE` and say so, or
  commit the file first. This is already the rule; the policy is why it exists.
- **Untracked evidence cited by a bead is unreproducible.** Pane 4 measured 25 untracked wave-2
  files that every convergence figure in `S1.toml` cited. The beads were individually sound and
  collectively invisible.
- **`git ls-files` reads the INDEX.** It reports files that are staged and not committed, so it is
  not proof that work landed. Verify with `git ls-tree -r HEAD`.

**NO-CLAIM.** This is a policy with **no gate behind it yet**. Nothing in-tree refuses a
`git worktree add`, and `.git/hooks/` cannot see one being created in another checkout. It is
enforced by every agent reading this file, which is exactly the enforcement class this repo
distrusts — and it stays that way until a hook or gate exists, which is a separate bead.

---

## Scratch homes

Work that outlives the command that created it MUST use the session-scoped
`ZS_SCRATCH` directory under `$HOME/.local/state/zeststream/scratch/<ntm-session>/<pane-or-agent>/<job>/`.
Do not create durable scratch under `/private/tmp` or `$TMPDIR`; those locations have no
session owner and cannot be safely reaped. Single-command ephemeral buffers MAY use
`mktemp` only when removed before the command exits. Use the `scratch-home` crate to
resolve, create, attribute, and age-reap jobs; missing or invalid owner metadata is
`UNKNOWN` and MUST NOT be auto-reaped.

---

## The second rule: BUILT ≠ WIRED

A mechanism that is written, tested, adversarially hardened, and **invoked by nothing** is worth
zero. Green tests on an unwired lane are not evidence; they are a receipt for work nobody consumes.

We take the mechanism from `franken_lean` (`crates/fln-conformance/tests/contract_roots.rs`, found
via `fh`), and it is the shape to copy:

```rust
/// Lanes that exist and are correct but are deliberately not yet wired.
const UNWIRED_LANE_ALLOWANCE: &[(&str, &str)] = &[];
```

**An empty allowlist.** Every lane must be wired; an exception is a *named row with a reason*, not
silence. A conformance test walks the declared lanes and fails on any that no caller invokes.

**In this repo that means:** every crate that declares a gate, check, or lane ships a test proving
a real caller reaches it — a CI job, a subcommand, another crate. `fh N043` is us failing this
exact way: *"BUILT ≠ WIRED aimed at ourselves, and we ran the full battery of verification rituals
without it firing once."*

**Wiring proof needs a positive control.** Grep for something you *know* is wired and confirm it
hits. A zero from a pattern that can never match is not evidence of absence.


---

## The third rule: no gate may exist without a reachable trigger

**We cannot afford an unwired gate, and the flag must be baked into the kernels — not into a
document, a log, or a CI file.** This rule is the standing one; read it every session.

A gate that cannot fire is worse than no gate, because the repo *reads* as protected. Measured on
2026-08-31, the flagship gate was correct — run by hand it exits 1, names the offending files, and
prints *"the exemption list is empty by design"* — and it was invoked **only** from
`.github/workflows/gate.yml`, in a repo that then had no remote. Two `.sh` files were committed into
the tree that gate forbids while it watched from a runner that did not exist.

### CORRECTION 2026-09-02 — the remote now exists, and the failure INVERTED into something worse

**Retracted:** *"`git remote -v` returns empty. The workflow can never execute."* and the census
row `no — no remote` on three gates. Both were true on 2026-08-31 and are false now:

```
$ git remote -v
origin  https://github.com/JYeswak/omp-orchestrator.git (fetch)
origin  https://github.com/JYeswak/omp-orchestrator.git (push)
```

`gh run list` shows **six consecutive FAILED runs, one per push** — `8e8050e`, `cb3df6d`,
`84833e0`, `2acf6a1`, `5520f42` and more. Root cause, from `gh run view --log-failed`:

```
error[E0554]: `#![feature]` may not be used on the stable release channel
error: could not compile `asupersync`
```

**`asupersync` requires nightly; the runner uses stable.** Every job that transitively depends on
it dies at compile — `omp-inventory-map`, `porting-gate`, `installer` and others — after burning
~17 minutes each.

**This is the INVERSE of the failure this rule was written for, and it is worse.** The rule warns
about a gate that *cannot* fire. What we have is a gate that **fires on every push, fails every
time, and nobody reads it** — while the local `PRE_PUSH_GATE_OK` receipt green-lit all eight of that
session's commits. **A local gate that certifies what CI rejects is a fooled certificate**, which
is strictly more dangerous than a silent one: it manufactures confidence instead of merely
withholding it.

So the rule needs a second clause. **Reachability is necessary and not sufficient. A gate must be
reachable AND its verdict must be READ.** An unread red is indistinguishable from an unwired gate
at the only point that matters — the moment someone decides the tree is fine.

Also corrected: a pane reported `gate.yml` as invalid YAML (*"Map keys must be unique at line 52"*).
It parses clean under a strict duplicate-key loader; line 52 is a well-formed `kernel-bypass-gate:`
job. **And note `yaml.safe_load` silently ACCEPTS duplicate keys, taking the last** — so a bare
`safe_load` cannot be used to disprove a duplicate-key claim. Use a strict loader.

**The census below is therefore STALE in the reachable column.** All six gates are now reachable
via a live remote. Re-derive it with `git remote -v` plus `gh run list` before citing it, and treat
the trigger column as the durable half.

**Why it lives in the kernel processes.** Every other signalling path here is *measured silent*:
`ATTENTION.txt` took 178 consecutive ticks from one writer with zero readers; the supervisor printed
a typed refusal naming `owner=josh` 29 times and nobody read it for hours; six red CI runs went
unread in a single evening. A file, a log, and a CI job are each loud in principle and silent in
fact. **The only path that reached a human was a nonzero typed outcome the operator had to answer.**

So the census is the **first** check in `decide()`, ahead of the pane and queue checks, and it is
unreachable-around: no branch may return `SupervisedWorking` or `AuthorizedIdle` while any gate is
unwired. These properties make it hold — **and the list is deliberately unnumbered in its
introduction**, because it read "Four properties" while carrying five for at least a day. A count in
prose is wrong the moment anyone adds a row, which is the defect the list itself is about:

1. **Absence of a census is itself a refusal.** `None => GateUnwired { CENSUS_NOT_PERFORMED }`.
   Nobody satisfies the supervisor by declining to look.
2. **Classify by trigger reachability, never caller existence.** `no-shell-gate` *had* a caller for
   hours while being unable to fire. A census keyed on "does something reference this" calls that
   WIRED and is wrong.
3. **Positive control is mandatory.** `no-shell-gate` must come back reachable, or the census is
   broken — one that reports everything unreachable is indistinguishable from one that works.
4. **The census must exclude its own source file.** Measured: a shell census reported `src-only` for
   all six gates because it grepped for gate names and *the census table names them all*. That is
   the self-referential checker — sixth instance in one session — where a checker's input contains
   text about the thing it checks.
5. **STRIP COMMENTS BEFORE MATCHING — a doc comment warning about a needle contained the needle.**
   Seventh instance, measured 2026-09-02 on `eg0m`. `ack-spine`'s census row keys on the emit site
   `ack_spine::ledger::step(` appearing in another crate's source, and the needle is assembled from
   parts so the checker's own code cannot contain it. Then the mutation that deletes the
   supervisor's only emit site **left the census GREEN**: the needle was still matching inside the
   doc comment two functions above, the one explaining why the needle is split. **The comment
   defeated the mitigation it documented.**
   Splitting the needle protects the checker from its own code; it does nothing about every OTHER
   file's prose, and a peer's comment mentioning a primitive would register as a caller. The
   general fix is to blank `//` and `/* */` before matching, exactly as **control-plane's**
   `close-evidence-gate` blanks fenced and inline code before harvesting paths (that crate is NOT in
   this repo — see the corrected section below). **Over-stripping is the safe direction** — it can
   only report LESS reachability, and the failure this prevents is a false GREEN.
   The lesson is not about comments. **A mutation that fails to bite is the most valuable result
   available**: the suite was green, acceptance 4 was satisfied on paper, and only deleting the
   subject showed the gate could not see it.

6. **THE TRIGGER FILE MUST PARSE, AND A PARSE FAILURE IS INDISTINGUISHABLE FROM A PASS.** Added
   2026-09-07 from `omp-orchestrator-m0c`. A census that asks *"is there a trigger"* answers YES for
   a workflow file that no runner can read. `%19` measured the consequence: a duplicate sibling key
   starts **zero jobs**, and a workflow that fails to parse **reports nothing at all** — so from
   outside, *"the gate fired RED"* and *"the workflow never ran"* are the same observation. That is
   the unread-red failure one step earlier: not a verdict nobody reads, but a verdict that was never
   produced.
   **Parse it with a STRICT loader.** `yaml.safe_load` silently ACCEPTS duplicate keys and takes the
   last, so it cannot be used to prove a duplicate-key claim either way. `m0c`'s own headline —
   *"`gate.yml` is invalid YAML (duplicate keys at `:46`/`:52`) so all nine CI gate jobs are
   unreachable"* — came from exactly that loader and is **false**: the file parses clean under a
   strict duplicate-key loader and `:52` is a well-formed job. The bead was right that an unparseable
   trigger is fatal and wrong that this one was unparseable.
   **And the reachability assertion must not pin a SHAPE.** `m0c`'s leg asserted a twelve-job
   structure, which was correct on 2026-09-06 — a missing job key had collapsed two jobs into one and
   silently halved coverage — and became a false RED the moment `fsu7` reduced the workflow to one
   entry point. The invariant was never *"this YAML has a job named X"*; it was *"X's gate is
   REACHED"*. Under twelve jobs those were one sentence; under one they are not, and only the second
   survives translation. **The structural half MOVES CRATES rather than disappearing** — reachability
   went to `gate-runner`'s subsumption census, which covers every invoked crate instead of one, and
   was proven to bite BEFORE the old leg was deleted: removing `state-wildcard-lint`'s
   `[package.metadata.gate]` stanza turns 3 of its 4 legs RED with *"its run half is UNREACHED"*.
   A deleted assertion whose replacement is untested is a coverage hole with a commit message. This
   is rule 10 aimed at a test instead of a ratchet: **an assertion keyed on an absolute count is red
   by construction the next time the thing it counts legitimately changes, and a gate that is red by
   construction gets routed around.**

**NO-CLAIM.** This makes an unwired gate **loud, not impossible**. Static reachability proves a
trigger exists; it cannot prove the gate *ran* this cycle. `tick-monitor` had callers and starved
the fleet for 4.5 hours anyway. And `.git/hooks/` is untracked and per-clone, so a fresh checkout on
another machine has **no hook** and `no-shell-gate` reverts to unreachable there — which is why the
census answers *for the machine it runs on* and says which one that is.

---

## The fourth rule: file → **claim** → dispatch, and never skip the middle beat

A packet naming an unclaimed bead is a dispatch that **the tracker never learned about**. It does
not appear as `in_progress`, `bv` cannot see it, no follow-up check can watch it, and when the pane
goes quiet nothing can distinguish *"the worker went silent"* from *"nobody was ever asked."*

**Measured 2026-08-31.** `5rh` was dispatched to `%1413` and never claimed. The bead sat `open`,
`assignee: none`, **zero comments**, while a worker was carrying it. The failure was silent in both
directions: the pane looked idle-with-no-assignment, and the queue looked like it still had ready
work nobody had taken.

**It also defeats the follow-up stage**, which is the part worth understanding. `classify_followup`
keys on *assigned + in_progress + no comment since dispatch*. An unclaimed dispatch produces **no
signal at all** — the detector built to catch dispatched-then-silent cannot see a dispatch that
never became tracker state. The bead's status is a *projection* of the dispatch; here the projection
was never written, so the watcher watched an empty slot.

```
file  →  claim  →  dispatch  →  observe  →  verify  →  close
         ^^^^^
         the beat that was being skipped
```

**The mechanical form:** the dispatch path must refuse to send a packet naming a bead that is not
`in_progress` and assigned to the receiving agent. A dispatch that cannot be projected into the
tracker is not a dispatch — it is a message.

**NO-CLAIM.** Claiming makes the work *visible*, not *done*. A claimed bead with a silent pane is
still a silent pane; this rule only guarantees the follow-up stage has something to look at. And an
`open`+unassigned bead is not by itself evidence of a skipped claim — it may simply be unstarted.
The signal is *dispatched* and unclaimed, which means the dispatch ledger, not the bead, is the
authority that closes this hole.

### ⛔ AND THE INVERSE IS WORSE: **CLAIM WITHOUT DISPATCH**, WHICH READS AS A LIVE WORKER

**Measured 2026-09-11, by the conductor, on a P0, while it was actively hunting this exact
class.** `br update … --status in_progress --assignee agent=bg-uldvu-2` landed; **the agent was
never spawned.** A tick arrived between the claim and the spawn, and the claim survived the
interruption while the intent did not.

**The skipped-claim failure above is LOUD — the bead sits `open` and the queue keeps serving it.
Claim-without-dispatch is SILENT and strictly worse:**

```
dispatch without claim   bead open, unassigned   -> queue re-serves it, someone notices
claim without dispatch   bead in_progress, held  -> queue SKIPS it, nobody notices, forever
```

A held bead is **removed from `br ready`**, so the one mechanism that would surface it is the
mechanism the phantom disables. **It is a self-concealing stall.**

**THE DISCRIMINATOR IS ONE COMMAND AND `br` CANNOT PROVIDE IT.** The assignee string is just
text; `hub list` is the liveness oracle. Cross the two:

```
assignee names an agent that is NOT in `hub list`   -> phantom, re-dispatch or release
```

This was the **seventh** phantom holder found in one session. Six were dead or invented names on
other agents' rows; **this one the conductor created itself, between two of its own tool calls.**

**THE MECHANICAL FORM: CLAIM AND SPAWN IN THE SAME TURN, AND VERIFY THE AGENT APPEARS.** Never
claim in anticipation of a dispatch you have not yet made — the window between them is exactly
where an interrupt lands, and what survives the interrupt is the *assertion* that work is
underway, not the work. Prefer spawning first and claiming from inside the worker, which cannot
produce this state at all.

### A DISPATCH-ONLY INSTRUCTION IS AN UNRECORDED REQUIREMENT

**The same rule, one layer up, and measured on the orchestrator 2026-09-02.** A dispatch packet
said: *"take that `NUMBERS.toml` row as part of this dispatch — it is directly in scope as a FIXED
finding, not a side errand."* The implementer's report never mentioned it. The bead was then graded
against **the bead's acceptance**, which never carried the item, and **closed.** Verified afterward:
`grep -ciE 'aggregate|--lib|per-suite' NUMBERS.toml` → **0**. The row was never added.

**The grader could not see the requirement — and the grader was the same agent that wrote it.** A
packet is a transient message; the bead is the durable spec. Every acceptance check keys on the
bead, so an item that exists only in a packet is invisible to verification by construction, no
matter who verifies.

**The mechanical form:** if a dispatch adds scope, it MUST be written into the bead's acceptance
**before the packet is sent** — `br update --acceptance`, then read it back. A packet may explain,
prioritise, warn, and name traps; it may NEVER be the sole record of something the work must do.

**NO-CLAIM.** This makes an added requirement *checkable*, not *done*. A bead can carry a perfect
acceptance list and still be closed by a grader who skips an item — which is what happened here, one
level down. And the reverse failure is real too: an acceptance list edited after dispatch can move
the target under a worker mid-flight, so the edit must precede the send, not follow the report.

### THE RECEIVER MUST **ANSWER**, AND NOBODY WAS EVER TOLD TO

**The lifecycle chain above is missing a beat, and its absence stalled the whole session.**
`ack-stage` admits exactly one form of authoritative delivery evidence: a comment on the dispatched
bead whose prefix matches, byte for byte,

```
ACK <token> on <pane_id> --
```

where `<token>` is the **last hyphen-segment** of the bead id (`omp-orchestrator-zrq` → `zrq`;
`omp-orchestrator-kxe.4` → `kxe.4`) and `<pane_id>` carries its percent (`%1413`). The parser is
`crates/ack-stage/src/lib.rs:243-253`; `:308-320` downgrades an otherwise-`ReceiptConfirmed`
delivery to `Indeterminate/AckReadbackMissing` when that comment is absent.

**The transport rule is deliberate, not a defect.** `:290-291` states that the tmux literal
fallback is **always** `INDETERMINATE` on receiver heuristics alone — a timer reset plus a
content-hash change is explicitly **not** accepted as proof of a uniform transport. So on that path
the ACK comment is not one evidence source among several; it is **the only one the design admits.**

**Measured 2026-09-02.** Every dispatch packet written this session omitted the instruction, and
**zero ACK-prefixed comments existed anywhere in the tracker.** `omp-orchestrator-zrq` → `%1413`
therefore ended `ACK_STAGE_INDETERMINATE / unproven_transport` while the packet had plainly landed:
**12 `zrq` terms in the pane against a positive control of 10.** The transport worked; the answer
was never requested. For hours this read as a transport defect (`cp-nq2s9`) when the gap was that
nobody had been told to reply.

**Why it hid is the same shape as the subsection above.** The sender half is a tested Rust crate.
The receiver half is a sentence in a hand-written markdown packet — which no gate reads, no test
covers, and no schema requires. A protocol whose two halves live in different media fails silently
in the medium that has no checker.

```
file  →  claim  →  dispatch  →  ACK  →  observe  →  verify  →  close
                               ^^^^^
                        the answer nobody asked for
```

**The mechanical form:** the dispatch site must emit the ACK instruction itself, so a human writing
markdown cannot omit it. Tracked as `omp-orchestrator-93lo`.

**RESOLVED IN PRACTICE 2026-09-05, and the fix was one sentence in the packet.** The claim above
that *"zero ACK-prefixed comments existed anywhere in the tracker"* is **no longer true** and must
not be cited as current. Three dispatches that carried the instruction as a **mandatory first line**
produced the first admissible delivery evidence this repository has ever recorded:

```
[WildStone] ACK iis6 on %9 -- grading by re-execution, not the implementer report.
[WildStone] ACK iis6 on %9 -- GRADE PASS MUTATION-VERIFIED. RRL6_VIOLATED=no:...
[WildStone] ACK gcyf on %9 -- DONE 102afde. Did not touch main.rs.
```

**Nothing in the sender changed.** `ack-stage` was correct the whole time; the packets simply began
asking. The measured cost of not asking was every dispatch of the prior session reading
`unproven_transport` while packets landed — which is why this row is worth more as a *correction*
than it was as a finding.

### AND THE CHAIN IS STILL MISSING A BEAT: **RELEASE**, BETWEEN VERIFY AND CLOSE

**Measured 2026-09-07, and it stalled three beads at once.** The orchestrator routed grades for
`f3g5`, `djfu` and `lppp` to non-authors while **the author still held `assignee`**. Two panes
refused inside one minute, in near-identical words:

```
status=in_progress  assignee=pane=%20;incarnation=1;agent=pane20-omp-claude
"The current holder is also the author, so an independent grade must wait for an explicit
 release/reassignment. Please release or reassign, then send the grade request again."
```

**Both refusals were correct and the routing bug was the dispatcher's.** An
implementation-complete bead assigned to its implementer is **not grade-ready however the packet is
worded** — and the packet said the right thing ("take it if unassigned; if `br` shows a holder,
tell me instead of forcing"), which is precisely why the wall was visible instead of being
bulldozed by a forced second claim.

```
file → claim → dispatch → ACK → observe → verify → RELEASE → close
                                                    ^^^^^^^
                                    the author hands the bead back before a
                                    non-author can claim it for grading
```

**The mechanical form:** the dispatch site must refuse to route a grade to a pane while the bead's
`assignee` names a different pane. The dispatcher is the only party positioned to notice, because
it is the only one that knows both the author and the intended grader. Reassignment is not a
workaround for this — it **is** the release beat, performed by whoever routes.

**AND AN AGENT NAME IS NOT AN IDENTITY.** The first correction set `--assignee WildStone`, which
**designates nobody**. Derived from every pane-scoped assignee string in the tracker:

```
%7   WildStone
%8   WildStone     ← one name, THREE panes
%9   WildStone
%19  PearlGate
%20  pane20-omp-claude
```

So this file's grading bar — *"DIFFERENT PANE, not different lineage"* — is **load-bearing rather
than stylistic**: `pane=` is the only unique field in an assignee string. A name is a persona shared
across panes; a lineage is shared across everything. Any eligibility check keyed on the agent name
cannot tell an author from a grader. Recorded on `xsu4` and `0luy`, which own author resolution.

**NO-CLAIM.** The release beat makes an independent grade **possible**, not **independent** — the
grader must still re-execute the acceptance rather than read the author's report. And the collision
census reads only assignee strings carrying the `pane=` form, so panes that never claimed a bead do
not appear and a pane renamed across incarnations shows as two rows rather than one collision:
**three-pane `WildStone` is a floor, not a total.**

### ONE ACK PER PACKET, NOT ONE PER BEAD — an N-bead packet serialises the fleet

**Measured 2026-09-07, and it is the conductor's defect, not a pane's.** I dispatched three batch
packets in one wave, each demanding an ACK **per bead** — six ACK comments. Every ACK is a
`br comments add`, every one takes `.beads/.write.lock`, and `.beads/beads.db` is **31 MB with four
live writers**. `%8`'s first ACK landed; its second timed out after `1m17s` waiting on **PID 15800 —
`%9` posting an ACK I had also demanded.** I serialised my own fleet behind a delivery receipt.

`%8` did the right thing twice: it **refused to kill a peer's `br` process**, and it reported the PID
and the wait instead of retrying blind. Second time in one session a pane correctly refused that
kill, and both times the report beat the kill.

**The ruling: a packet needs ONE ACK.** The ACK exists to answer *"did the packet arrive"* — because
on the tmux path `ack-stage` admits only a matching bead comment, a timer reset plus a content-hash
change being explicitly insufficient (`crates/ack-stage/src/lib.rs:290-291`). **One ACK from a pane
answers that completely.** A second ACK for a second bead in the *same* packet raises no evidence
tier; it only multiplies write-lock contention on the critical path.

**Unchanged per bead:** the verdict, the re-run evidence, which tree each number came from, and the
`MUTATION-VERIFIED` / `DONE` / `APPROVED` / `WONTFIX` prefix with the status read back. **This
relaxes the delivery receipt, never the acceptance evidence.**

The rule immediately above says *"the dispatch site must emit the ACK instruction itself"* — correct,
and as written it invited a per-bead reading, which is what I built the packet from. **The dispatch
site must emit exactly one ACK token per packet.**

**NO-CLAIM.** This removes ACK amplification from the dispatch path; it does **not** fix the
contention. ~31 MB for ~939 beads is ~32 KB each, reads run 40–250 s against a 30 s default, and one
`br list --limit 4000` full scan starves every writer. The write discipline stands: long evidence to
a file, a one-line pointer in the bead, `--lock-timeout 60000` on every call, and **read the
output** — a suppressed `br update` failure is indistinguishable from success and silently lost two
claims in one session.

#### THE PACKET'S ONE ACK TOKEN MUST BE A REAL BEAD TOKEN — a nickname is unmatchable

**Caught by `%9` 2026-09-07, in my own rule, one packet after I wrote it.** It ACKed and reported:
*"`ACK s1ratify on %9 --` (on `omp-orchestrator-jplf.7.2`; token bead `s1ratify` does not exist)."*

**The token is mechanical, not a label.** `crates/ack-stage/src/lib.rs:293-298`:

```rust
fn ack_token(bead_id: &str) -> &str { bead_id.rsplit('-').next().unwrap_or(bead_id) }
fn ack_prefix(bead_id: &str, pane_id: &str) -> String {
    format!("ACK {} on {pane_id} -- ", ack_token(bead_id))
}
```

and the match is `strip_prefix` at `:507` against `format!("ACK {} on ", ack_token(bead_id))` — an
**exact prefix**. So a packet nickname can never match, and neither can a decorated token:

```
bead omp-orchestrator-typed-blocker-taxonomy-report-redispatch-zey6   token = zey6
  ACK zey6 on %9 --          MATCHES
  ACK zey6-grade on %9 --    NO MATCH   (extra text before " on ")
  ACK s1ratify on %9 --      NO MATCH   (no bead ends in -s1ratify)
```

**Measured over `.beads/issues.jsonl` — the JSONL, because `br list --json` omits comments
entirely; my first audit returned a blind `0`. CITATION CORRECTED 2026-09-07:** this was originally
attributed to `close-evidence-gate`'s `source.rs:223`, **a control-plane crate that does not exist
in this repo**. The defect is real HERE and `%19` proved it locally at
`crates/pre-delete-citation-check/src/lib.rs:194`, whose own body reads *"The br JSON does not
inline comments; the caller fetches them separately"* — **and no caller ever did**, both production
callers passing `Vec::new()` at `:198`. Measured: `br list --json --status closed` → **196 rows, 0
carrying a `comments` key**; the JSONL → **195 rows, 182 carrying comments**; **14 closed beads
cite a `bin/` or `.flywheel/` path ONLY in comments, one of them the bead that created the citation
gate.** Fixed and wired at `crates/no-shell-gate/src/bin/pre-commit-gate.rs:342`
(`read_closed_beads_from_mirror`), verified by pane 1 at the live hook crate.

**This names a variant not previously recorded here: BUILT ≠ WIRED at FIELD granularity.** Not an
uncalled crate — an unfilled struct field whose consumer runs on every commit. `ClosedBead::comments`
existed, `check_deletions` scanned it, and the callers handed it an empty vector, so the gate could
never catch the incident named in its own header. A crate-level wiring census cannot see this; only
reading what the caller passes can.

```
ACK comments matching the ack-stage prefix : 415
ACK comments UNMATCHABLE by construction   :  37   (8.2%)
  ACK selector on %9      on a bead whose token is 2ceb
  ACK reap on %9          on 3r1r
  ACK childoutcome on %9  on 7kxf
  ACK reroute on %9       on 93lo
```

**So the protocol is 92% healthy and the 37 are a real, bounded leak** — not the catastrophe the
first look suggested. Every one of the 37 reads to `ack-stage` as `AckReadbackMissing` and downgrades
its delivery to `Indeterminate/unproven_transport`: **the dispatch landed, the work happened, and the
evidence is unusable.**

**And the one-ACK-per-packet rule above MADE THIS WORSE, which is why the two clauses ship
together.** Per-bead ACKs were correct by construction — each token came from its own bead. By
collapsing to one ACK and naming it after the *packet*, I detached the token from any bead at all.
**A dispatcher writing a batch packet MUST pick one bead from the batch and use its bare token.**

**NO-CLAIM.** This fixes the token's *shape*. It does not make an ACK proof of progress — an ACK is
a delivery receipt any agent can type, and `:290-291` still holds the tmux path at
`INDETERMINATE` on receiver heuristics alone. And 415 matching ACKs is not 415 verified dispatches;
it is 415 parseable ones.

**And the correction is the load-bearing part, per this file's own rule about stale doctrine.** A
doctrine row asserting a mechanism is broken *licenses routing around it indefinitely*. Left
uncorrected, this section would have kept telling readers the ACK path produces nothing, long after
it started producing everything — the identical failure mode as the `refill-idle-panes` row that
claimed a kernel carried control-plane paths for hours after it had been rebuilt clean.

**What it bought beyond a receipt.** The `iis6` ACK carried `RRL6_VIOLATED=no` with an argument
against the law it might have broken, `TREE_PINNED=yes`, and a `NO_CLAIM` that explicitly declined
to credit the implementer for an uncommitted half. The ACK line is where a grader's *reasoning*
becomes checkable, not merely its arrival — so the structured tail (`MUTATION_RED=`, `SHA=`,
`POSITIVE_CONTROL=`) is not ceremony. It is the field that caught `WIRED_TO=dispatch_packet` naming
a caller that does not exist.

**STILL OPEN, and it is the whole mechanical fix.** Every one of those ACKs happened because a human
wrote the instruction into a hand-authored packet. `omp-orchestrator-93lo` — emit the instruction
from the dispatch site — remains **unlanded**, so the protocol still depends on the conductor
remembering. Three ACKs prove the receiver half works when asked; they prove nothing about the next
packet a tired operator writes.

**NO-CLAIM.** An ACK proves the packet **arrived and was read** — nothing about the work. Acceptance
evidence is still the bead's own criteria, re-run by a grader who is not the implementer. And an ACK
is forgeable by construction: it is a comment any agent can write, so it is a *delivery* receipt,
never a *progress* one.

### THE INPUT MANIFEST IS A FIELD, NOT PROSE

Any tool, census, or grade result used as evidence MUST carry a required InputManifest field. The field has exactly three states: FULL, PARTIAL with bound_kind, bound_value, and source, or REFUSED with reason. There is no Default implementation: omitting the manifest is a construction error, not an implicit FULL.

An empty scan or result set is the typed EmptyScanSet error, distinct from FULL with zero rows and distinct from PARTIAL. PARTIAL and REFUSED results are non-citable acceptance evidence; the grade-ingestion boundary MUST reject them before a bead can close. A non-recursive or otherwise bounded instrument MUST emit PARTIAL with its bound named, or REFUSED; it MUST NOT silently slice or answer an empty set.

The mechanical form is deliberate: the manifest is a serialized struct field on the emitted artifact, not a log line, comment, or convention. Source paths from state files and pane transcripts are tagged SelfReferentialCorpus; a corpus containing only those hits MUST NOT report FULL. This is the neighbouring rule to file -> claim -> dispatch: the result carries what input was actually consumed before anyone cites it.

NO-CLAIM: a manifest records the instrument's declared input coverage. FULL does not prove the subject result is correct, and static source coverage does not prove a runtime invocation.

---
## The fifth rule: the crates exist to orchestrate OMP, and today they scrape it

Everything in this repo is built to drive OMP. Measured 2026-08-31 against the **installed** source
at `/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent` (v18.0.11, `dist/cli.js` 19 MB),
the crates consume **none of it**. Not a thin subset, not a legacy subset — zero. Every `dist/…` path
below is relative to that install root.

**Two columns, same day. Every number has the command that produces it:**

| surface that exists | command that counts it | measured | we consume |
|---|---|---|---|
| CLI subcommands | `omp --help`, COMMANDS block | **39** | **0** |
| type-surface directories under `dist/types` | `find dist/types -mindepth 1 -maxdepth 1 -type d \| wc -l` | **57** | **0** |
| top-level declaration files beside them | `find dist/types -mindepth 1 -maxdepth 1 -name '*.d.ts' \| wc -l` | **14** | **0** |
| an RPC transport ships — `--mode=<text\|json\|rpc\|rpc-ui>` is a documented top-level flag | `omp --help \| grep -- --mode` | **1 flag, 4 modes** | **0** |
| `omp/*` methods in the bundle | `grep -oE '"omp/[A-Za-z]+"' dist/cli.js \| sort -u` | **3** — `omp/muxConnect`, `omp/muxPing`, `omp/muxRestartServer` | **0** |

57 + 14 = **71 entries** under `dist/types`. Say it that way. An earlier pass published "71
directories"; a worker independently measured 57 and the two disagreed. The reconciliation was that
entries had been counted and called directories. **Neither number was fabricated — the noun attached
to the count was wrong**, which is the same class as every other confident-wrong figure here. The
directories that *are* our lifecycle are named in that tree: `jsonrpc`, `tools`, `slash-commands`,
`commands`, `session`, `task`, `goals`, `plan-mode`, `modes`, `subprocess`, `exec`, `dap`, `debug`,
`capability`, `registry`, `extensibility`, `memories`, `mnemopi`, `memory-backend`, `irc`, `collab`,
`live`, `eval`, `hindsight`, `autolearn`, `autoresearch`, `security`, `secrets`.

**The zero is four greps over `crates/*/src/*`, each printed with the count it returned:**

~~~bash
cd /Users/josh/Developer/omp-orchestrator
for p in 'Command::new("omp")' 'mode=rpc' 'muxConnect' 'omp/'; do
  printf '%s -> %s files\n' "$p" "$(git grep --no-index -lF "$p" -- 'crates/*/src/*' | wc -l | tr -d ' ')"
done
~~~

Measured output — `Command::new("omp")` → **0 files**; `mode=rpc` → **0**; `muxConnect` → **0**;
`omp/` → **0**.

**Positive control, per the second rule.** The identical command shape with a pattern we know is
present returns nonzero: `Command::new("br")` → **3 files**. A zero from a pattern that can never
match is not evidence of absence. `--no-index` is load-bearing, not cosmetic:
`crates/no-shell-gate/src/bin/pre-commit-gate.rs` is untracked, so tracked-only `git grep` reports
**3** `git` spawn sites where the working tree has **4**.

What the crates *do* spawn, same census:

~~~bash
git grep --no-index -hoE 'Command::new\("[a-z_-]+"\)' -- 'crates/*/src/*' | sort | uniq -c | sort -rn
~~~

```
   5 Command::new("br")
   4 Command::new("git")
   1 Command::new("tmux")
   1 Command::new("cargo")
```

`br`, `git`, `tmux`, `cargo`. **No `omp`.** We orchestrate OMP by reading the terminal it drew.

### ⛔ CORRECTED 2026-09-10 — EVERY ONE OF THOSE FOUR ZEROS IS NOW FALSE, AND THE POSITIVE CONTROL HAS INVERTED

**Retracted:** *"`Command::new("omp")` → **0 files**; `mode=rpc` → **0**; `muxConnect` → **0**;
`omp/` → **0**"* and *"**No `omp`.** We orchestrate OMP by reading the terminal it drew."* Both were
true on 2026-08-31. Re-measured today by pane 1, running the file's own commands:

```
                          raw   code_only     <- code_only = same grep with // comments stripped
Command::new("omp")         4       2
mode=rpc                   28      14
muxConnect                  2       1
omp/                       48      32
```

**We consume OMP over a typed transport now.** `ompo state --json` returns **437,558 bytes** of live
OMP state with `adopted_method: get_state`; `ompo stats` and `ompo messages` project session cost and
message roles the same way. The two real spawn sites are
`crates/omp-surface-consumption/src/main.rs:60` and `crates/ompo-doctor/src/omp_process.rs:238`.

**READ THE `code_only` COLUMN, NOT THE RAW ONE — and the reason is this section.** The raw counts are
inflated by comments *about the old zero*: `crates/omp-surface-consumption/src/lib.rs:39` reads
``//! `Command::new("omp")`, `mode=rpc`, `muxConnect`, `omp/` — all **0 files**.`` A grep for the
needle matches the prose recording that the needle was absent. That is the **self-referential
checker** this file already names in census property 5 (*"a doc comment warning about a needle
contained the needle"*) — firing here on the census that produced the doctrine. Strip `//` and
`/* */` before matching; over-stripping is the safe direction, since it can only report LESS
consumption.

**THE PRESCRIBED POSITIVE CONTROL NOW RETURNS ZERO, SO THE RECIPE ABOVE IS BROKEN AS WRITTEN.**
`Command::new("br")` → **0 files**, against the **3** this section cites. `br` did not stop being
spawned; it moved behind the kernel — `crates/finding/src/lib.rs:58` declares
*"Tracker CLI the bead-filing kernel owns. Callers use `Command::new(finding::BR)`"*, and the spawn
at `:463` is `Command::new(self.program.clone())`. **A literal-argument grep cannot see a spawn whose
program is a const or a field**, which is the same blind spot as `ripwire --uses` missing call sites
inside `assert!()`. So anyone re-running this census gets a control that reports absence for a live
caller and cannot distinguish a real zero from an instrument zero — rule `8i`'s failure condition
reached by following rule `8i`'s recipe. **Pick a control that is still a string literal** (measured
today: `Command::new("git")` → 33, `Command::new("ps")` → 10, `Command::new("cargo")` → 10) **and
prefer `ripwire --uses` or the compiler over a text count.**

**The live spawn census, replacing the four-row block above:**

```
  33 git · 10 ps · 10 cargo · 8 sleep · 4 timeout · 4 omp · 3 crontab · 3 am · 2 tar
   2 shasum · 2 sh · 2 pgrep · 2 ntm · 2 launchctl · 2 kill        (0 br — see above)
```

**WHAT THIS DOES *NOT* RETRACT, and it is the load-bearing half.** The rule's *point* was never the
integer; it was that scraping paint is not a protocol. That still stands, and this file's own
`HD-0049` block measures the residue: **115 spinner-regex sites across 11 crates**, five NTM robot
verbs at zero consumption. The zeros are gone; the scrapers are not. **Do not read this correction as
"the fifth rule is satisfied"** — read it as "the denominator moved and the numerator is no longer
zero." Re-derive both before citing either.


**Every classifier defect measured today is downstream of this one fact** — not correlated with it,
caused by it. Each row names what we do instead of a protocol, and why the protocol makes the defect
unconstructible:

| defect, measured 2026-08-31 | what we do instead | why a protocol removes it |
|---|---|---|
| pane state | a **braille-spinner regex** over `capture-pane` | a state method exists; a spinner is a *rendering*, and we are parsing paint |
| "receiver receipt" | **timer reset + spinner-stripped content hash** ≥75s apart | a typed send returns a delivery response; a hash of glyphs is a guess about one |
| two codex panes read `<no marker>` | last-status-line scan, defeated by a **tool-call box border drawn AFTER the status line** | an artifact of *draw order*. Draw order does not exist over a typed protocol |
| `ntm --robot-send` refuses codex panes with *"cod composer not visible"* (cp-nq2s9) | a **terminal-inspection guard** | a protocol refusal names a *state*; this one names a **visibility**, which is a fact about pixels |
| cp-z42vu: a send returned `success:[4]` while the packet never arrived — and the **inverse** fired today in the pending-dispatch marker | fire-and-hope | both directions are the signature of an **unacknowledged transport**. Ack removes both, not one |

**Of the surface we do not consume, the split that matters** (measured independently and agreeing):

- **(b) historical reimplementation by scraping — 4:** pane state, dispatch, session, health check.
  For profiled OMP panes, pane state now has the typed profile-store route below; scraping remains
  only for third-party or bare-shell panes with no OMP profile/session mapping.
  These historical rewrites are not evidence that the corresponding protocol is an attach route.
- **(c) should use — 2:** `goals`, `collab`. Nothing in `crates/` mentions either.

**CORRECTION 2026-09-08 — the three `omp/*` names formerly listed in the installed-surface row are not pane orchestration.** The
installed OMP bundle defines `__omp_worker_lsp_mux`, `OMP_LSP_MUX_SOCKET`,
`OMP_LSP_MUX_PROJECT_DIR`, `omp.lsp.mux`, the ready banner `omp lsp mux listening on \S+`,
and `pong`; its Unix address is `path.join(dir, "lsp-mux.sock")` (with a Windows named-pipe
alternative). This is a language-server multiplexer for OMP workers, not a tmux-pane/session API.
The two observed workers (PIDs 43609 and 75508) held anonymous inherited socketpairs; no
`lsp-mux.sock` existed on disk and `OMP_LSP_MUX_SOCKET` was unset. Those methods are therefore
not an attach route for the existing tmux server and must not be advertised as one.
**Current pane-state route (measured 2026-09-08): use the profile-scoped terminal-session index as
an input, not as proof of liveness.** For a pane `%N`, read the profile from the pane's own
`omp --profile` argv, then read `~/.omp/profiles/<profile>/agent/terminal-sessions/tmux-%N`.
The entry's line 2 is the last session opened, not necessarily the live agent's session. Validate
that it is under that profile's `agent/sessions/` root before passing it to the state-only OMP
reader; otherwise the pane state is `UNKNOWN`. A valid mapping retires spinner parsing without
inventing an endpoint.

**Terminal-session entry contract is observed, not assumed.** The format is variable-length: line 1
= cwd, line 2 = candidate session JSONL path, and optional line 3 = a state token. Across the 64
current entries (Claude 22, Codex 42), the only observed token is `fresh` (14 entries), while 50
entries have no line 3. The current Codex `tmux-%8` entry points outside the profile's agent session
root at a scratch conformance fixture, so it is not proof of the live pane session. A reader MUST
preserve missing line 3 as `Unknown/MissingStateToken`, never infer `fresh`; an entry whose line
2 is outside the profile root is `Unknown/ForeignSessionPath`; tokens not observed remain `UNKNOWN`.

**Route decision:** choose the validated profile-store reader for existing panes. `omp acp` is a
real JSON-RPC protocol for supervisor-owned subprocess sessions, but it is point-to-point and cannot
attach to an already-running pane. A new pane-keyed mux would buy a separately addressable,
push-capable endpoint, but no current consumer needs that beyond a validated reader; adding one now
would duplicate supervision, socket lifecycle, and recovery without adding observed capability. The
profile-store route is not an endpoint and must return `UNKNOWN` when its candidate session is not
validated as belonging to the profile. Revisit a mux only when a consumer requires remote
subscriptions or control unavailable through the reader.

This decision is version-bound to installed OMP 18.1.14 and the observed machine state; re-derive it
after upgrades or profile changes.

`omp-orchestrator-omp-surface-map-41b` owns turning this into the per-crate table.

**NO-CLAIM.** "No crate calls OMP" is measured **for our crates only** — the four greps above scan
`crates/*/src/*` in this repo and nothing else. **NTM may itself speak an OMP protocol beneath
`--robot-send`; that is UNMEASURED.** The evidence leans against it — a protocol-level refusal would
not be phrased as *composer visibility*, and a protocol-level receipt would not be reconstructed from
a timer reset — but leaning is not measuring. Until someone reads NTM's send path, the honest claim
is about the boundary we scanned.

**NO-CLAIM, second.** **Mapping a surface is not adopting it.** Some of the scraping is likely
*correct*: a third-party pane (codex, a bare shell) has no OMP RPC to answer, so terminal inspection
is the only channel that exists for it. This rule does not say "replace the scraper." It says the
choice must be **visible** — for each scraped surface, either the typed alternative is named and not
used for a stated reason, or it is used. Silence about a 71-entry surface we touch zero times is the
failure, not the scraping.

---

## OMP lifecycles — the three that matter

**Receipt:** [`CENSUS-ARCHIVE-OMP.md`](docs/inventories/CENSUS-ARCHIVE-OMP.md#omp-lifecycles--what-they-are-and-where-to-find-them)
— the RPC state table, the v18 status-line contract, and the installed-surface census. **Its
integers are archived and NOT citable.**

**Read the enum, not a table, when precision matters** — `crates/xtask/src/omp_rpc.rs` in
control-plane is the authority for the RPC machine.

**Three properties carry the weight and they are rules, not description:**

- **A terminal state admits no further transition**, and the machine enforces it, not the caller.
- **A restrictive terminal is one a caller MUST NOT read as success** — `Failed` and `TimedOut`.
  **So a timeout is not a verdict:** an empty buffer from a killed child maps to `TimedOut`, never to
  the token a genuinely failing subject would produce.
- **No wait is unbounded, including shutdown.**

**The pane lifecycle is read from a terminal and is wrong more often.** Three rules:

- **Read the LAST status line, never the buffer** — a whole-buffer scan matches a stale spinner in
  scrollback, and one pane scored working AND idle simultaneously while genuinely idle.
- **Two captures or it is not a claim** — `Working (27s)` and a frozen pane render identically.
  Compare the timer **and** a spinner-stripped content hash ≥75s apart.
- **`safe_to_dispatch` is not liveness.** A wedged pane accepts a packet, parks it at
  `Press up to edit queued messages`, and never submits it.

**The bead lifecycle:** `open → in_progress (claimed) → closed (with cited evidence)`, with two
traps that are ours. **A close reason must start `MUTATION-VERIFIED` / `DONE` / `APPROVED` /
`WONTFIX`** — **but NOTHING REFUSES a prose reason today, and the earlier text here claiming
otherwise is retired.** `ack-spine` types the four prefixes and names its own bypass; measured
2026-09-08, **6 of 241 closed beads carry a non-conforming prefix and one is EMPTY.** So the prefix
is a convention you keep, not a wall that catches you — **always read the status back.** Tracked as
`uqnut`, whose measured decision is to EXTEND the set: five of those six (`PREMISE-FALSE`,
`MUTATION-NOT-REQUIRED`, `MUTATION-ATTRIBUTED`) are MORE precise than the four, so forcing them
behind `WONTFIX` would delete information a grader deliberately recorded.
**And a child blocked by its parent epic cannot close**, which inverts the dependency and
makes both permanently unclosable; `--force` with the reason recorded is correct when the epic is the
only blocker — **and ONLY then.** Measured the same day: `a92y` and `gfb` were each blocked by a real
open TASK, two panes refused `--force` on both, and both refusals were right.

## The four skills, and how they compose here

Not four checklists — one philosophy with four entry points. Each has one sentence that binds.

### `/planning-workflow` — converge before you build
**Plan-space is ~25× cheaper than code-space.** Debates belong in planning, before the swarm burns
implementation tokens. Three reasoning spaces: plan (architecture, cheapest to change), bead (task
boundaries, ~5× plan to rework), code (~25× plan). Don't answer plan-space questions in code-space.

### `/beads-workflow` — the bead is the spec
**"Check your beads N times, implement once."** Every bead is self-contained with **testable
acceptance: run X, expect Y**. A bead you cannot write acceptance for is not granular enough.

*Why this is load-bearing here:* a bead without acceptance cannot be **worked**, only **adjudicated**
— and adjudication reliably produces "no work to be done" instead of work. Measured: a P0 bead at
the head of the ready queue had **no ACCEPTANCE section at all**; two agents in a row triaged it and
went idle rather than shipping.

### `/beads-bv` — the DAG decides the lane
PageRank over the dependency graph. **Work the articulation points, not the comfortable leaves.**
Easy-bead cherry-picking while critical-path work starves is a named pathology, not a preference.

### `/vibing-with-ntm` — observe before you nudge, and police the credit
Two rules, both binding:

> **One Rule.** A pane is not stuck, idle, limited, blocked, or finished until pane truth, robot
> state, work state, and artifact evidence **agree**.

> **Second Rule.** The swarm is paid in credit and will counterfeit credit if you let it. Process
> artifacts are not progress, refusals are not delivery, commits are not a KPI, and a close without
> cited evidence is a debt.

**DO NOT POLL. BLOCK.** Repeated activity checks on a timer are the anti-pattern; NTM ships blocking
waits that fire on a state transition. Tails verify a post-condition on one pane — they never
*discover* that something changed.

### `/brennerbot-with-ntm` — delete hypothesis space, don't accumulate evidence

> A session is **a machine for deleting hypothesis space cheaply**, not a machine for accumulating
> evidence. Maximize (expected mind-change × downstream option value) / (time × cost × ambiguity).
> When two phases compete, the one that kills more candidate hypotheses per token wins.

**No falsifier means no session.** Prefer **refuters over supporters** — evidence that could kill
your hypothesis is worth more than evidence consistent with it. Generate ≥3 hypotheses including a
**forced third alternative**, then attack the survivors.

*Applied here:* when a lane misbehaves, write the falsifier first. Tonight's dispatcher bug survived
three rounds of hypothesising and died in one `bash -x` — because the trace could refute, and the
theories could only agree with themselves.

### How they compose

```
/planning-workflow   converge the plan      ─┐
/beads-workflow      plan -> testable beads  ├─ before any agent is dispatched
/beads-bv            DAG says which bead    ─┘
/vibing-with-ntm     dispatch + police credit ─── during the wave
/brennerbot          when something is wrong  ─── falsifier first, refuters over supporters
```

---

## The crates — what they are, and WHICH REPOSITORY they are in

**Receipt:** [`CENSUS-ARCHIVE-CRATES.md`](docs/inventories/CENSUS-ARCHIVE-CRATES.md#the-crate-extraction-target-list--what-each-one-is-and-which-repository-it-is-actually-in)
— the 24-row historical extraction table, the per-crate source audit, the dependency shape, and the
`pane-truth` specimen. **Every figure in it is archived and NOT citable.**

**DO NOT CITE A PACKAGE COUNT FROM THIS FILE. RUN THE COMMAND.** The figure moved
`27 → 50 → 51 → 65` inside the lifetime of this document, and each stale value was corrected by an
agent who then wrote a fresh integer that went stale in turn. **The correction is not a better
number — it is no number:**

```bash
cargo metadata --no-deps --format-version 1 --offline \
  | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["packages"]))'
find crates -mindepth 1 -maxdepth 1 -type d | wc -l      # these two MUST agree
```

**DERIVE TOPOLOGY FROM `cargo metadata`, NEVER FROM `grep`.** A shell census reported 1 leaf where
the resolver reports 33, because `grep -c … || echo 0` emits `"0\n0"` — and a Rust `count()` returns
a `usize` that cannot. **That is the concrete reason this repo forbids shell rather than
discouraging it.**

**Extract leaves first** — a crate with zero intra-workspace path deps ports without dragging a
second crate across the repo boundary.

**THERE IS NO PERCENT-PORTED FIGURE AND THERE CANNOT BE ONE.** The extraction scope was asserted as
20 crates and as 23; **neither figure ever shipped a producing command.** With the numerator moving
hourly and the denominator never established, any percentage, burndown or "N of M" claim is
unfounded. **Treat 20 and 23 exactly as the retired "81 JSON-RPC methods, 17 used" pair: cite
neither.**

**A `CONTROL-PLANE` row in the archive is not evidence a crate is absent here.** Measured: the table
calls `loop-driver` control-plane; it exists in **both** repos and the copies have **diverged**.
Check with `find crates -maxdepth 1 -type d -name '<name>'` before believing either.

## Use fh before you build anything

`fh` is the queryable index over our own measured doctrine and Jeffrey's 180-repo mirror. **Ask it
before writing a crate, a gate, or a process.** Re-deriving what we own is the largest token waste
this fleet has.

```bash
fh suggest "<what you are about to build>"   # ranked rows
fh why <row-id>                              # provenance before you believe it
```

Four row types answer different questions:

- **CAPABILITY** — depend on this crate instead of writing it; names what of ours it replaces
- **DOCTRINE** — a measured failure with path + quote + line; the mistake we already paid for
- **BEAD** — current task intent with exact `.beads/issues.jsonl#id` provenance
- **DOC** — pinned repository guidance with source revision and verbatim evidence

**Rows already governing this repo:**

| Row | Governs |
|---|---|
| `C38` | A fixture drifted from production certifies nothing — its green is indistinguishable from a working check |
| `C112` | An ownership claim must name something that **dies with the thing it owns** — a pid in a marker file written by a transient shell dies with the shell |
| `N043` | BUILT ≠ WIRED aimed at ourselves: a full battery of verification rituals that never fired once |
| `N040` | A replacement claim needs a smoke check at **both** ends — the crate installs clean **and** the old caller is gone |

`fh` reports a `STALE` banner when its ledger is older than its threshold. **Read it and say so** —
a stale row is still evidence, but its age is part of the citation.

**AND THE VERIFIER DOES NOT EXAMINE `N` ROWS AT ALL, SO THE TWO ROWS THIS FILE LEANS ON HARDEST ARE
UNVERIFIED — NOT FALSE, UNVERIFIED.** Measured 2026-09-07 by `%19`, reproduced independently in a
**425,512-byte** payload:

```
fh verify ledger --json   success=false  code=DRIFT  exit=5
  FAILED            22 rows   C124 C127 C133 … C175 C57 S8
  CANNOT_DETERMINE  33 rows   C117 C118 … C177 C178
  EVERY row in both lists is C### or S##.  ZERO N rows in either.

control, because ABSENCE is the thing being read:
  C124 mentioned=True   C57 mentioned=True   S8 mentioned=True   <- known-failed rows DO appear
  N046 mentioned=False  N043 mentioned=False                     <- N rows never do
```

**So `N043` not appearing in the drift set is not evidence that `N043` verified.** It is outside the
verifier's scope — the vacuous-absence trap, the same shape as `closure-check` returning
`worst=Pass` for roots it could not see. **This file cites `N043` as governing in three places
(`:67`, `:216`, and the table above), and every one of those citations rests on an unverified row.**

**A ROW ID IS `fh`'s INDEX KEY AND NEVER PROVENANCE — proven from the source end.** In
`~/.claude/references/franken-harvest.md`: `grep -c N046` → **0**, `grep -c N043` → **0**, while the
quoted text itself sits at **`:722`**. The citable form is **file + line-range + the quote**, never
the row id:

```
~/.claude/references/franken-harvest.md:718-725
  "Producer-side invocations from /tmp using the long binary name do exist, and were
   deliberately not counted: they prove testing, not consumption."
```

**`fh why` resolves LEDGER rows and refuses CODE rows, and the refusal is typed rather than empty:**
`fh why N046` returns the row plus its body; `fh why TECH:meta_skill:40fa2f6fc247:1089251` returns
`[EMPTY/WHY_QUERY_NO_MATCH]`. **A `TECH:` row carries its own provenance in the search output — the
`repo@sha` and the `path:line-range`. That is the citation.** Read the body on the mirror at that
revision; `fh suggest` tells you **where to look** and cannot tell you **whether to copy** (rule
`8j`).

**`fh doctor --json` reports `DRIFT` exit 5 (`ACTIVE_GENERATION_PRODUCT_INPUT_DRIFT`) on dirty
source input**, so treat fh as **retrieval and provenance context, not a clean repo grade** —
`%7`'s scoping, adopted. **And fh discloses its own limit in the same payload**, to its credit:
*"This result reports the requested operation only; it does not establish correctness, completeness,
or causal success."*

---

## The asupersync contract (binding)

Every subprocess — `tmux`, `ntm`, `br`, `bv`, a build — is cancellable work with a deadline. Built
on **asupersync 0.4.9** (`/Volumes/ZestData/dicklesworthstone-mirror/asupersync`, a real `[lib]`).

- **`&Cx` first** in every async API we own; `cx.checkpoint()` in loops, retries, long handlers.
- **Region-owned tasks** (`Cx::spawn` / `Scope`). **No detached tasks.**
- **Kill the process GROUP, never the pid.** Measured: orphaned grandchildren (`ppid=1`, 0.0% CPU)
  held the admission lock, so every timeout guaranteed the next attempt failed too — the failure
  created the condition for its own repetition.
- **Drain both pipes.** Undrained stdout+stderr with a `try_wait()` poll deadlocks past ~64 KiB.
  The tell is **0% CPU with no children**; widening the timeout hides it longer.
- **A timeout is not a verdict** — see the restrictive terminals above.

Load `/asupersync-mega-skill` before touching spawn, cancellation, or scheduling code.

---

## Every gate proves it bites

1. **Fires-on-known-bad.** A gate that has never fired on a bad input is not evidence of anything.
2. **A known-GOOD leg is mandatory.** An attack-only suite ships an over-strict gate, and an
   over-strict gate gets routed around — a slower death than no gate.
3. **A mutation leg.** Break the thing the gate keys on; the leg must go RED. Restore
   byte-identically. If it stays green, the leg is not attributable and proves nothing.
4. **Anti-vacuity.** An empty scan set is an **ERROR**, never a pass. A deliverable never checked
   reports identically to one that passed.
4a. **A SURVEY THAT FINDS NOTHING MUST SAY WHICH NOTHING** (`fh C69`, doctrine, inserted `3e09f5a`,
   amended `1bf70ce`; harvest was STALE — `digest_missing_today` — when cited on 2026-09-07):

   > *"no such mechanism exists here"* and *"the mechanism exists and was not exercised"* are **two
   > verdicts with two different remedies**, and a survey that emits one colour for both sends the
   > reader to the wrong repair.

   Three outcomes, three colours, never merged:

   ```
   ABSENT       the thing does not exist        -> build it
   INERT        it exists, nothing invokes it   -> WIRE it   (N043; do NOT rebuild)
   UNRUN        it exists and is wired, not run -> run it    (never a pass; see 4 above)
   ```

   **Measured 2026-09-07, on this repo's own authorization.** I searched the tracker for the six S1
   layer gates using `CONTRACT.md`'s spelling, got `ABSENT` six times, and reported the
   authorization as pointing at nothing. They existed under `gate-s1-l0-…`; the contract had
   transcribed `l0` as `10`. The correct verdict was **INERT** — all six present, all `open`, and
   **zero beads wired to any of them.** ABSENT says *write the gates*; INERT says *attach the work*.
   I nearly dispatched the first.

   **The discriminator is a positive control on the matcher itself.** `gate-s1-djn8` and
   `gate-s2-ehx8` both returned PRESENT from the same query, so the query worked and the six really
   were missing *under that spelling* — which is exactly how a structurally-guaranteed zero passes
   as a measurement. A token-level search (`jtgw` → 4 hits) is what separated ABSENT from INERT.
   **Before reporting absence, search for a FRAGMENT of the name, not the whole name.**

4b. ⛔ **A POSITIVE CONTROL DRAWN FROM THE POPULATION IT VALIDATES IS VACUOUS — AND AN EXISTENCE
   PREDICATE IS NOT A REACHABILITY PREDICATE.** Added 2026-09-11 after this defect was measured
   **twice in one night, in two different crates**, the second time inside the supervisor's own
   anti-vacuity machinery. **This is rule 4 defeated by a predicate that can never return empty.**

   ```
   omp-orchestrator/src/lib.rs:684   coverage_output_reachability
       -> Reachable { trigger } WHEN crates/<name>/Cargo.toml .is_file()
   omp-orchestrator/src/lib.rs:558   derived_positive_control
       -> .find(|r| r.reachability.is_reachable())          <- the FIRST such row
   ```

   **Measured: ALL 11 `COVERAGE_WAVE_OUTPUT_CRATES` rows satisfy it trivially**, so the row that
   proves *"this census verified SOMETHING"* is satisfied by **any directory on disk with a
   manifest in it**, and the supervisor believed `dispatch-silence-watch` reachable for a reason
   unrelated to whether anything calls it.

   ⛔ **AND THE SPECIMEN THIS RULE FIRST NAMED WAS WRONG — CORRECTED WITHIN THE HOUR BY THE AGENT
   IMPLEMENTING IT, WHO WOULD OTHERWISE HAVE PINNED A FALSE VERDICT INTO THE REPAIRED ORACLE.**
   The original text called `crate-atom-gate` *"the crate rule 10 records as having never been
   invoked by anything."* **That misreads rule 10 by one axis.** Measured:

   ```
   manifest callers  3   no-shell-gate · omp-inventory-map · orchestration-tick-gate
   source uses       3   kernel-bypass-gate · no-shell-gate/src/bin/pre-commit-gate.rs
                         orchestration-tick-gate/src/main.rs
   ```

   **Rule 10 says its GATE has never FIRED** — the pre-commit call site sits behind a disarmed
   `OMP_CRATE_ATOM_GATE=1` flag. **Its LIBRARY is used by three crates.** A correct reachability
   predicate MUST call it Reachable, so it is the **known-GOOD**, not the known-bad. The
   known-bad has to be a **synthetic** crate — manifest and nothing else — which is both the
   shape the old predicate provably cannot fail **and** one that cannot drift when a peer adds a
   dependency.

   **THE REUSABLE PART IS THE MISREAD, NOT THE CRATE.** *"The gate never fires"* and *"nothing
   uses the crate"* are **different axes**, and a doctrine sentence compressing them licenses the
   next reader to assert the stronger one. That is the borrowed-claim rule (*a borrowed claim
   inherits its author's burden*) firing on a claim borrowed **from this file, by its own
   author,** one rule later.

   **THE TWO SHAPES, and they compose into a gate that cannot fail:**

   - **EXISTENCE ≠ REACHABILITY.** `Cargo.toml` exists for every crate by construction, so the
     predicate partitions nothing. Per rule 9 part 2, reachability means a trigger that can
     **FIRE** — a workflow entry, a hook, a crontab/launchd row, or a `Command::new` spawn from
     something that itself has one.
   - **THE CONTROL MUST COME FROM OUTSIDE THE POPULATION.** Picking the first passing row as the
     proof-of-non-vacuity means the control and the subject share a failure mode: **when the
     predicate is wrong, the control is wrong in the same direction and cannot report it.** Same
     defect as rule 7b's overlapping mutation sets, and as `pd5ua`'s `docs/gate-roster.txt`
     matching **11 of 11** failing crates *because it is the full 88-crate roster* — ⛔ **A SOURCE
     THAT CONTAINS THE WHOLE POPULATION CANNOT EVIDENCE A SUBSET OF IT.**

   **THE CHEAP TEST, and it is one row:** name a subject you KNOW should fail and assert the
   predicate rejects it. If you cannot name one, the predicate is not discriminating — that is
   `8i`'s two-probe control aimed at a classifier instead of an instrument.

   **Found by an agent that REFUSED to anchor a wiring proof on the repo's existing oracle and
   reported why, rather than hand-rolling around it silently.** Routing around a broken kernel is
   what keeps it broken (see KERNEL-ONLY); reporting it is what fixes it.

5. **State the claim as a floor-raise.** Say what the gate mechanically enforces *and* what still
   passes. A residual "guarantees / proves / makes impossible" in a gate header is itself a defect —
   the overclaim is worse than the gap, because a reader stops looking.
   For structure-keyed census work, route source through `text-structure::code_only` (or `manifest_deps`) and run `no-shell-gate/tests/text_structure_lint.rs`; the lint is the caller-facing refusal for new text-keyed checkers.

6. **An `#[ignore]`d leg is not a passing leg.** Measured 2026-09-02:
   `findings_ledger::real_findings_ledger_is_strictly_valid` — the ONLY leg that validates the
   actual `FINDINGS.jsonl` rather than a fixture — is `#[ignore]`d, and `--include-ignored` has
   **zero callers** in this repo (the only matches are inside a vendored `asupersync` checkout
   under `.rch-tmp/`). It passes when run. So a default `cargo test` reports the gate green while
   production data goes unchecked: BUILT ≠ WIRED at test granularity. If a leg must be opt-in,
   something in-tree has to opt in.

7. **A known-bad leg must assert its MESSAGE *AND* its exit code — pinning either one alone is
   defeasible, and BOTH failure directions are measured.** Measured 2026-09-02.
   `DJ-D-OT-UNGATED` rests on `cargo test -p no-shell-gate --test orchestration_tick` returning
   **101 / "no test target named `orchestration_tick`"**. During an unrelated fleet outage — a
   `Cargo.toml` under the `crates/*` glob with no `src/`, which breaks workspace **loading** so
   `-p <crate>` cannot dodge it — that identical command **also returned 101**, with a completely
   different message. **Same exit code, different cause.** Anyone re-running that Validation block
   mid-outage would have seen 101, ticked the box, and confirmed the finding on the wrong evidence.
   `101` is `cargo`'s generic failure; a fires-on-known-bad leg matching only `rc != 0` goes green
   on any unrelated breakage. Grep the specific string, and print the output so a reader can see
   which cause fired.

   **AND THE CONVERSE IS ALSO MEASURED — 2026-09-07, which is why this rule now says AND.** The
   sentence above used to read *"assert its MESSAGE, not just its exit code"*, whose natural reading
   is **message INSTEAD OF code**. `%19`'s M1 mutation on `2sx1` defeats that reading directly:

   ```
   assertion `left == right` failed: a publisher returning no id must exit 5, not the
   missing-field code: FINDING_PUBLISH_FAILED detail=br create returned success without an id
     left: Some(3)   right: Some(5)
   ```

   **The token stayed correct while the code collapsed.** `FINDING_PUBLISH_FAILED` was still
   printed, so a leg pinning only the MESSAGE would have stayed **GREEN** under a mutation that
   merged two distinct causes into one exit code. `%20`'s leg pinned both and caught it.

   So the two directions are symmetric and neither assertion subsumes the other:

   |mutation|code|message|caught by|
   |---|---|---|---|
   |unrelated breakage (workspace-load outage)|same `101`|**differs**|message|
   |two causes collapsed to one code|**differs** `5→3`|same string|code|

   **Pin both, or the leg is defeasible in one direction you have not named.** `%19` found this
   against a bead it was grading and reported it as a correction to *our rule*, not as a pass —
   which is the behaviour the grading gate exists to produce.

   **This is the sixth instrument defect of that session, and they are one family:** `$?` after a
   pipe returning the pipeline's status (three false findings, all in surfaces being audited FOR
   false success); a `timeout` ceiling below the subject's documented 5-minute deadline read as
   "unbounded" (three agents, one shared wrong model, each measuring their own SIGTERM); a
   `cargo metadata` probe whose `map(select($n|index(.)))` asked whether a list contained itself;
   a pane-identity probe whose input contained every agent name because the operator had typed them
   all; the flagship pre-commit gate exiting **0** on an empty index while the worktree was dirty at
   30 files. **In every case the instrument produced the reading, not the subject.** A gate's own
   evidence is subject to this too — which is why a leg must pin the message and the code together,
   rather than trusting whichever one it happened to look at first.

   **AND A PREFIX IS NOT A MESSAGE — the crate filed to demonstrate this rule got it half-wrong.**
   Measured 2026-09-07 by `%7` grading `djfu`: the mutation trap pins the wrong DECISION and the
   `SALVAGE_UNKNOWN` **prefix**, but never asserts the exact reason text. The mutation it was built
   for — *"still printed `HOLD` in its reason while deciding `RelaunchAsIs`"* — survives a prefix
   assertion, because the prefix is unchanged. **Assert the substring that the mutation actually
   moves**, or the message half is decorative.

   **⛔ AND PINNING BOTH IS THE WORKAROUND. SPLITTING THE CHANNEL IS THE FIX.** Added 2026-09-10
   from `GradePxhmd`, generalising four instrument over-reads measured in one evening. Rule 7 as
   written tells you to READ HARDER. That is correct and it is second best, because it leaves the
   ambiguity in place for the next reader.

   **Every one of these is ONE CHANNEL CARRYING TWO CAUSES:**

   ```
   tvu5      rc=103   "the build failed"      vs  "the shim refused, with no diagnostic"
   eux9p     rc=75    "the verb is refused"   vs  "the BYPASS is refused"   (gate is inside
                                                   `if RCH_CARGO_WRAPPER_BYPASS == 1`)
   rustfmt   rc=1     "does not parse"        vs  "is not format-clean"
   awk       delta≠0  "unbalanced delimiters" vs  "parens inside strings and comments"
   ```

   **The remedy that actually ends it is a WIDER CHANNEL, and this repo already shipped one.**
   `pxhmd`'s citation step had exit `4` meaning both *"gh is unavailable"* and *"the run is still
   in progress"*. The fix was not a more careful reader — it was `EXIT_SELF_REFERENCE=9`,
   `EXIT_RUN_IN_PROGRESS=10`, `EXIT_LOCAL_AGGREGATE_UNAVAILABLE=11`. Three causes, three codes,
   and the over-read became unconstructible. Compare `e0klo`, where `AdapterStatus` gained a
   fourth variant rather than teaching readers that `Live` sometimes means foreign; and `6636a`,
   where `ReapLockHolder::{NoHolder, ProbeUnavailable}` replaced the single token `"unknown"`.

   **So the rule has two tiers, and prefer the second:**

   1. **As a CONSUMER** you cannot change the channel: pin the message AND the code together, and
      say which cause you observed.
   2. **As the AUTHOR of the signal** you can: give each cause its own code or variant. An
      assertion that must read two fields to disambiguate is telling you the emitter under-typed
      its outcome.

   **The outstanding instance is `exit=75` itself** — one code for *"bypass refused on Darwin"*
   while fleet doctrine reads it as *"the verb is refused"*, which is `eux9p`. A distinct code for
   the two would retire the misreading permanently instead of documenting it. That file is
   SUBSTRATE, so the remedy is an upstream report, never a local patch.

   **NO-CLAIM.** Splitting a channel removes ONE ambiguity; it does not make the emitter honest.
   A gate can emit nine precise codes and still pick the wrong one, and `e0klo` is the proof that
   the variant existing is not the same as the aggregate consuming it — that defect was a typed
   field recorded and then ignored by the summary that counted it.


7b. **TWO MUTATIONS PROVE INDEPENDENCE ONLY IF THEIR FAILURE SETS ARE DISJOINT.** Measured
   2026-09-07: `%20` reported *"two mutations redden DIFFERENT leg pairs, so the code assertions and
   the decision assertions are provably independent."* `%7` re-ran both and found the pairs
   **overlap**:

   ```
   M1  Unknown => 20 -> => 0     RED: no_evidence_at_all_is_unknown_with_a_distinct_code…
                                      an_unattributable_session_log_holds_rather_than_guessing
   M2  Hold -> RelaunchAsIs      RED: no_evidence_at_all_is_unknown_with_a_distinct_code…
                                      unknown_never_recommends_a_relaunch_across_every_shape
   ```

   **One test reddens under both**, so it asserts the code property and the decision property at
   once and cannot discriminate between them. **An overlap does not merely fail to prove
   independence — it PROVES at least one test conflates two properties**, which is the defect the
   mutation pair was supposed to rule out.

   The remedy is to partition: one leg asserts the code and says nothing about the decision, another
   asserts the decision and says nothing about the code. Then a disjoint redden is a measurement
   rather than a claim.

   **This is the same family as a correlated oracle** — `riqd`'s leg 3 found two "independent"
   pane-state channels that derive from the same capture and therefore agree when wrong. **Two
   probes sharing a subject are one probe.** Ask what each mutation is allowed to touch, and check
   that the sets differ before calling them independent.

7c. ⛔ **A MUTATION LEG'S *POSITIVE* ARM CAN BE VACUOUS: A SECOND MECHANISM CAN RESTORE THE
   EXPECTED STATE AFTER THE MUTATION REMOVES THE FIRST, SO THE LEG STAYS GREEN ON THE EXACT
   DEFECT IT EXISTS TO CATCH.** Measured 2026-09-11 on `pane-dispatch-ready`'s
   `mutation_busy_markers_load_bearing`, fixed at `1c311ed`, and **proven by running it rather
   than argued**.

   The leg asserts a rule is load-bearing: delete the rule, the verdict must change. It asserted
   that by comparing a STATE (`state_of(&on) == "BUSY"`). With the rule deleted, classification
   fell through to `FREE` — and **an absent composer then fail-closed it straight back to
   `BUSY`**. Two different causes, one observable, so the assertion could not tell *"the rule
   fired"* from *"the rule is gone and something else failed closed."*

   **A FAIL-CLOSED DEFAULT IS THE CLASSIC MASK, and it is the one to look for**, because
   fail-closed is otherwise correct design: it makes the safe state reachable by two paths, and a
   state-only assertion cannot name which path it took. **Assert the REASON, not just the state**
   — the same lesson as rule 7's *pin the message AND the code*, aimed at the positive arm rather
   than the negative one.

   **THE DISCRIMINATOR IS FREE AND NOBODY RUNS IT: apply the mutation and check the arm you
   expect to STAY GREEN, not only the one you expect to redden.** A leg whose positive arm is
   green both with and without the subject is measuring nothing, and it reads exactly like a leg
   that works.

   **AND THE ROOT CAUSES BENEATH IT ARE A REUSABLE TRIO**, all three in one crate:
   `which <binary>` produced EMPTY stdout on a host lacking it and that empty string was passed
   on as an env var; `std::env::var()` returns `Ok("")` for an empty variable, **silently
   suppressing a whole discovery ladder** — every sibling read in that same file already filtered
   empty, this was the one that did not; and the classifier returned its missing-dependency arm
   unconditionally, so the fail-closed rule governed an unknown exit code but not an absent file.
   ⛔ **`var()` ACCEPTING `Ok("")` IS AN ABSENT-vs-EMPTY COLLAPSE** — rule 4a's distinction living
   inside an environment read.

8. **A `cargo` figure is NEVER evidence about a commit.** `cargo test` reads the **WORKTREE**; a
   commit sha names a **TREE**. In a shared checkout those diverge constantly, so a grade that
   cites a sha and a test count has silently mixed two tree states. Measured 2026-09-02, against
   the orchestrator: it graded commit `5b164cd` and reported `admission-reason` at 22 passed / 3
   failed with a `rc=3` behavioural defect. Both were artifacts of uncommitted work by a third
   agent. `git log --oneline 5b164cd..HEAD -- crates/admission-reason` -> **0 commits**;
   `git status --porcelain` -> three files modified; `#[test]` count **16 at HEAD, 17 in the
   worktree**; `ExitCode::from(3)`/`ledger_missing` **0 occurrences at HEAD, 2 in the worktree**.
   At the commit under grade the test passes and the crate has 16 tests. The implementer's original
   figures were right and the grader's were right — **for different trees**.

   **This is the mirror of the `git ls-files` rule and it bites in the opposite direction.**
   `ls-files` reads the INDEX, so it gives a false GREEN on "did my work land"; verify with
   `git ls-tree -r HEAD`. `cargo` reads the WORKTREE, so it gives a false RED (or a false green)
   about a commit. To grade a commit you must pin the tree: read it with `git show <sha>:<path>`,
   diff with `git log <sha>..HEAD -- <path>`, and state which tree every number came from. A grade
   is otherwise a claim about "the repo right now", which is not a thing five agents can agree on.

**RECEIPTS FOR EVERY ROW BELOW —
[`INSTRUMENT-DEFECTS.md`](docs/negative-patterns/INSTRUMENT-DEFECTS.md), 35,829 bytes,
byte-identity proven.** The rule binds here; the evidence that earned it lives there. Extracted
2026-09-07 by `%19`, which re-extracted by **content anchor** after this file grew under it
mid-pass — `8g` firing on the extraction of `8g`.

**Per-rule receipts, every anchor verified by OBSERVED RENDER (`pandoc -f gfm -t html`), not by slug
derivation** — a derived slug was wrong twice (`8f` preserves the underscore in `project_excluded`;
`8h` takes a TRIPLE hyphen because its title opens with `` ` M` ``):

[8b](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8b--a-retraction-that-lives-only-in-a-report-gets-re-proposed-by-the-next-reader-put-it-in-the) ·
[8c](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8c--an-exclusion-record-with-no-corresponding-live-row-is-stale-state-not-contention--and) ·
[8d](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8d--a-test-name-is-a-claim-and-a-name-promising-a-property-it-does-not-check-is-the-quiet-form) ·
[8e](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8e--the-lane-sends-your-worktree-for-tracked-paths-only--so-rch-e410-has-two-causes-and) ·
[8f](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8f--rch-i005-project_excluded-has-three-cases-not-two--and-rch-queue-cannot-tell-you-which) ·
[8g](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8g--a-broadcast-is-a-snapshot-and-carries-no-timestamp-a-reader-checks) ·
[8h](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8h---m-alone-is-not-a-collision-signal--m-with-nonzero-insertions-is) ·
[8i](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8i--run-a-negative-control-on-your-instrument-before-you-believe-its-answer) ·
[8j](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8j--fh-suggest-never-returns-empty-so-a-suggest-row-is-a-candidate-not-a-hit) ·
[8k](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8k--ripwire-emits-one-line-so-every-line-filter-deletes-the-whole-payload--and-a-miss-is-not) ·
[8l](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8l--a-successfalse-from-ntm---robot-send-is-not-proof-of-non-delivery-and-the-retry-may) ·
[8m](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8m--recording-an-observation-can-destroy-the-evidence-for-it-if-your-write-touches-the) ·
[8n](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8n--git-add----path-is-path-scoped-and-still-whole-file-it-does-not-isolate-you-from-a-peer) ·
[8o](docs/negative-patterns/INSTRUMENT-DEFECTS.md#8o--grep--c-symbol-counts-occurrences-including-the-uses-inside-the-thing-you-are-deleting)

8b. **A retraction that lives only in a report gets re-proposed by the next reader.** Put it in the
   source and give it a test.

8c. **An exclusion record with no live row for your project is STALE STATE, not contention** —
   unpin and go. Superseded in detail by `8f`.

8d. **A test name is a CLAIM.** A name promising a property the body does not check is the quiet
   form of the conjunctive defect; rename it to what it asserts.

8e. **The lane sends your worktree for TRACKED paths only, so `RCH-E410` has two causes:** a path
   absent everywhere, or a path present locally and untracked. **`git add` before blaming the lane.**

8f. **`RCH-I005 project_excluded` HAS THREE CASES WITH THREE OPPOSITE REMEDIES, and `rch queue`
   cannot tell you which.** The table IS the rule — dropping any case re-creates the wait `8c` was
   written to end. **Read the project field.**

   ```
   no live row for the project        STALE       -> unpin and go
   a live row that is YOUR OWN build  CONTENTION  -> wait for yourself
   a live row that is a PEER's build  CONTENTION  -> request another worker
   ```

8g. **A broadcast is a SNAPSHOT and carries no timestamp a reader checks.** True-when-measured is
   not true-when-read; **re-derive before acting on a peer's figure.**

8h. **` M` alone is not a collision signal; ` M` with NONZERO insertions is.** Discriminate with
   `git diff --numstat -- <path>`: **`0 0` means no peer is editing it.** But **`--numstat` answers
   *"is a peer editing this"*, NOT *"is the exec bit set"* — proven 2026-09-07: on a content-dirty
   file, `chmod 755` then `chmod 644` leaves numstat **identical at `86 7` both ways.** For the mode
   question use **`git diff --summary`**, which `--numstat` cannot show on a content-dirty file.

8i. **RUN A NEGATIVE CONTROL ON YOUR INSTRUMENT BEFORE YOU BELIEVE ITS ANSWER.** Joshua,
   2026-09-07, fleet-wide. **This is the parent rule: it subsumes `8b`–`8h` and is the lens for
   `8j`–`8p`.** It is only actionable with the two-probe form, so the recipe stays here:

   ```
   <tool> --root /nonexistent/path/xyz ; echo "rc=$?"     # what does ABSENT look like?
   <tool> --root . ; echo "rc=$?"                         # now the real one
   ```

   **Indistinguishable → the instrument cannot answer your question. Fix it before reporting
   anything.** A denied, errored, empty or unreadable probe is `UNKNOWN`, never a negative result.

   **⛔ AND 8i HAS NO PURCHASE WHEN YOU DO NOT THINK YOU ARE HOLDING AN INSTRUMENT. A STATED HOLD
   IS AN INSTRUMENT TOO.** Added 2026-09-10 from `GradeParity`, after the rule failed on the pane
   enforcing it.

   Measured: `GateRunnerNames` told pane 1, in words, that it was **deliberately withholding a
   push** so an in-flight CI run could finish as a data point. Pane 1 then ran

   ```
   git log origin/main..HEAD --oneline     # 3 commits unpushed
   ```

   and pushed. `concurrency: cancel-in-progress` killed run `34548539101` at step 7 of 8, ~20
   minutes in. **`git log origin/main..HEAD` answers WHAT IS UNPUSHED. It cannot answer WHO IS
   DELIBERATELY WAITING, and that second fact existed only in the channel where the peer said it.**

   Pane 1 had spent the whole session correcting exactly this defect in others — one channel read
   as though it carried a second — and did not catch it here, because **it never occurred to pane 1
   that it was reading an instrument at all.** You run a negative control on a `grep`; you do not
   think to run one on a queue state, because a queue state does not feel like a measurement. It is
   one, and it has a scope like any other.

   **THE EXTENSION:** before an action that is irreversible or that pre-empts a peer, ask **WHAT
   FACT WOULD STOP ME, AND WOULD THE THING I JUST LOOKED AT CONTAIN IT?** A tracker state, a queue
   depth, a monitor's `dispatchable` list, and a peer's stated intention are all channels with
   scopes. The cheapest form is one line to the peer, which costs seconds against a build that
   costs twenty minutes.

   **NO-CLAIM.** This does not make pre-emption impossible — a peer can hold silently, and nothing
   in the tracker records "waiting on purpose". It removes only the case where the peer DID say so
   and the conductor did not treat the saying as evidence.

8j. **`fh suggest` NEVER returns empty, so a row is a CANDIDATE, not a hit.** `fh search` returns
   `[EMPTY]` for a true miss and is the negative control — **but a STALE index returns
   `[STALE/SEARCH_INDEX_STALE]` for BOTH verbs, so the control cannot discriminate.** Measured
   2026-09-07: `fh search` and `fh suggest` on a guaranteed-absent needle returned the **same
   banner**, same key. **Confirm freshness with `fh health` before trusting either as a control**,
   or you are inside `8i`'s failure condition while following `8i`'s recipe.

8k. **`ripwire` emits ONE line, so any line filter deletes the whole payload** — and a structural
   miss is not an absence, because macro-argument names are invisible to `--uses`. Read raw, follow
   up with `--grep`, and pass `--legend=compact`.

8l. **A `success:false` from `ntm --robot-send` is indeterminate in BOTH directions and the retry
   may double-paste.** Read the JSON payload, never the exit code.

8m. **Recording an observation can destroy the evidence for it.** If your write touches the field
   you are citing, **capture the value FIRST.**

8n. **`git add -- <path>` is path-scoped and STILL WHOLE-FILE: it does not isolate you from a peer
   editing the same file.** When ` M` shows nonzero insertions (`8h`), stage your hunks with
   `git apply --cached` against HEAD content, then **COMMIT FROM THE INDEX WITH NO PATHSPEC** —
   `git commit -- <path>` re-reads the WORKTREE and undoes the isolation you just built.

8o. **`grep -c <symbol>` counts OCCURRENCES, including the uses inside the thing you are deleting.**
   Use the compiler's dead-code warning, or `ripwire --uses`, which attributes to the enclosing
   symbol.

8r. **THE CHEAP PARSE ORACLE — AND ITS FIRST THREE FORMS WERE ALL WRONG.** Added 2026-09-10. A
   parse error in any file of a crate fails every `cargo` verb for that crate, so tonight one pane
   spent **574,889 ms of a Contabo slot** discovering an unclosed delimiter. `rustfmt --check` is
   a **free local parse oracle** — it is not a build, so the `exit=75` refusal does not apply. But
   the obvious form is defective in three independent ways, each found only by asking what the
   *previous* fix still could not see:

   ```
   rustfmt --edition 2021 --check <path> >/tmp/rf.txt 2>&1 ; grep -c '^error' /tmp/rf.txt
                                                             ^^^^^^^^^^^^^^^^ ALL THREE BUGS
   ```

   **(1) A MISTYPED PATH SCORES A PASS.** `rustfmt` prints lowercase `error:` for a parse failure
   and **capital-`E` `Error:`** for a missing file, so `^error` cannot see a path that never
   resolved — the instrument reports identically to a clean tree. Measured:

   ```
   src/lbi.rs   (TYPO)     rc=1   '^error' = 0   '-ciE ^error' = 1    "Error: file ... does not exist"
   src/lib.rs   (clean)    rc=1   '^error' = 0   '-ciE ^error' = 0
   unclosed delimiter      rc=1   '^error' = 1   '-ciE ^error' = 1    <- the known-bad leg
   ```

   **`-i` is the rung-1 equivalent of `Remote command finished: exit=`**: it makes *did not run*
   and *did not parse* report through the same nonzero, so a zero finally means what the rule
   claims. Swept over all 91 crate roots it flips **zero** answers, so the widening introduces no
   false positive — and leg D above proves it still fires on a real parse error.

   **(2) `rc` IS OVERLOADED FOUR WAYS AND HERE IT INVERTS** — the NONEXISTENT file returns `rc=1`
   while a genuinely clean file returns `rc=0`, and a merely unformatted-but-valid file also
   returns `rc=1`. **Read the `^error` count. Never `rc`.**

   **(3) THE CRATE ROOT IS NOT THE CRATE — IT MISSES EVERY INTEGRATION TEST.** `tests/*.rs` are
   **separate crate roots**; no `mod` path connects them to `lib.rs`. Proven by planting an
   unclosed delimiter in `crates/ompo-doctor/tests/l1_doctor.rs` and restoring it byte-identically:

   ```
   run on src/lib.rs      total errors = 0   <- BLIND to the planted defect
   run on all 15 targets  total errors = 1   <- catches it
   ```

   `ompo-doctor` has **15 targets**: 1 lib · 1 bin · 12 test · 1 custom-build. A crate-root green
   covers `lib.rs` + 12 modules and says nothing about 13 other files — while being
   byte-indistinguishable from a green covering all of them.

   ✅ **THE COMPLETE FORM. Derive the roots; never choose one by hand:**

   ```bash
   cargo metadata --no-deps --format-version 1 --offline \
     | jq -r '.packages[]|select(.name=="<crate>")|.targets[].src_path' \
     | while read -r p; do
         rustfmt --edition 2021 --check "$p" >/tmp/rf.txt 2>&1
         grep -ciE '^error' /tmp/rf.txt
       done
   # sum == 0  =>  every target parses AND every path resolved
   ```

   **1,165 ms for all 15 targets of `ompo-doctor`** — about 1/13th of one `cargo check`, and
   `cargo metadata` is explicitly allowed under the build refusal. This also **retires
   root-selection-by-judgment**: you do not decide whether the root is `lib.rs` or `main.rs`,
   cargo tells you it is both plus thirteen more. The bin-with-zero-`mod`s trap
   (`ompo-doctor/src/main.rs` declares **zero** file-backed mods, so a green there covers ONE
   file) dissolves rather than needing a workaround.

   **IF YOU CITE THE PATH LIST, USE `--files-with-diff`** — it emits bare paths with no `:LINE:`
   suffix, which makes the lines-vs-files conflation **unconstructible** instead of merely warned
   against. Three agents reported `13 paths` for output containing **3 files** (`lib.rs` 11 hunks
   + 1 + 1) because a `sed`/`sort -u` over the un-deduplicated form dedupes HUNKS, and then built
   a false <!--RETIRED-->"the output is non-stationary between reads"<!--/RETIRED--> rule on the disagreement. Three reads were
   **byte-identical** (one `sha256` across all three).

   **DENOMINATORS, since four different true counts circulated for one crate:** `ompo-doctor` =
   **15 targets** / **12 file-backed modules** / **13 `mod` declaration lines**. Workspace =
   **91 crates** / **165 `lib.rs`+`main.rs` files** / **179 including `src/bin/*.rs`**. All true,
   none interchangeable — state which you counted.

   ⛔ **RESIDUAL, AND IT IS THE ONE THE THREE FIXES DO NOT COVER: 8r's GREEN IS EXACTLY AS
   COMPLETE AS ITS `cargo metadata` IS FRESH.** Found by `GradePxhmd` 2026-09-10, inside the
   corrected rule. `-i` catches a `src_path` that has been **DELETED** — that path prints
   capital-`E` `Error:`. It cannot catch one that was **NEVER LISTED**, because a target added
   since the metadata read emits nothing at all. Same shape as every other defect here: one
   channel, two populations — *targets that exist* versus *targets cargo listed*. So the honest
   statement of a green is **"every target cargo knew about when metadata ran"**, and
   `--offline` reads a cache. Re-run metadata in the same command as the sweep, never from a
   variable set earlier in the session.

   **NO-CLAIM.** This proves the crate **PARSES**, which is strictly weaker than type-checks and
   far weaker than compiles. It is a pre-check that stops you buying a remote slot to learn a
   delimiter is unbalanced; `cargo check -p <crate>` (~15 s local, also not refused) is the next
   rung and the only one that answers a type question.

8s. **THREE RESTORE PRIMITIVES, THREE DIFFERENT SEMANTICS — AND A MUTATION LEG WANTS THE ONE
   ALMOST NOBODY USES.** Added 2026-09-10 after the conductor broadcast the wrong one twice and
   three agents corrected it independently.

   Every gate rule here demands a mutation leg that restores **byte-identically**. The acceptance
   text across this repo says `git checkout -- <path>`, which is **unexecutable** — `dcg` refuses
   it as `core.git:checkout-discard` — and a worker following it verbatim reads the refusal as
   its own error. But the substitute matters more than the blockage:

   ```
   cp aside, then cp back    restores the WORKTREE as it was    allowed      <- CORRECT for a mutation
   git show HEAD:<path>      restores the TREE at HEAD          allowed      <- READBACK ONLY
   git checkout -- <path>    restores the INDEX                 dcg-BLOCKED  <- and a peer moves the index
   ```

   **A mutation leg is asking "is the worktree exactly as I found it", and only the first answers
   that.** Measured the same day: **125 dirty worktree entries, 87 modified-tracked.** In that
   checkout `git show HEAD:<path>` **silently discards a peer's uncommitted work in the same file
   while you believe you restored** — the whole-file hazard that also produced a 9-hunk sweep in a
   path-scoped commit that same hour. `git checkout` is worse still: it restores from the INDEX,
   which is shared state a peer can move under you mid-leg.

   **HOUSE FORM:** `cp` the file aside **BEFORE** mutating, restore from the copy, prove with
   `sha256`. `git show HEAD:<path>` keeps exactly one job — **READBACK**, proving what landed in
   the tree.

   **THE RULE IS A PRECONDITION, NOT A PROHIBITION** — amended the same day after two agents
   showed their tree-restores were provably safe and a blanket ban would have outlawed a legal
   readback. `git show HEAD:<path>` restores the **TREE**, so it is a valid restore **only when
   the worktree already equalled the tree**. Establish that FIRST with
   `git status --porcelain -- <path>` returning empty, or `cp` aside. One case where both are
   equivalent, and it is common: a file **added by your own commit minutes earlier** — HEAD and
   worktree are the same bytes, so there is nothing any peer could lose. With 87 dirty tracked
   files the `cp` form is the right DEFAULT precisely because that precondition usually fails.

   **AND `git diff --numstat` IS NOT AN ORACLE ON AN UNTRACKED FILE** — it is vacuously empty
   there, so a restore "proven" that way proves nothing. Run it only against a tracked path, and
   prefer `sha256` or `cmp`, which do not care about tracking state.

   ⛔ **AND `numstat`-ZERO IS A VALID RESTORE PROOF *ONLY WHEN THE FILE IS OTHERWISE CLEAN* —
   THE OBVIOUS GENERALISATION OF ITS OWN CORRECT USES IS UNSOUND.** Added 2026-09-11 by the agent
   whose two earlier proofs invite the wrong reading. On `cq4fb` and `poumg.6` it restored a file
   its commit did not otherwise touch, so `git diff --numstat -- <path>` → `0 0` was a sound
   restore proof. On `poumg.5` the same command reads **`70 85` and MUST NOT be zero**: the
   mutated file is **legitimately changed by that very commit**, so a zero there would mean the
   fix had vanished.

   **The two situations are indistinguishable from the command's output alone**, which is why the
   proof must be `sha256` + `cmp` against the `cp`-aside copy in BOTH. `numstat` answers *"does
   the worktree differ from HEAD"*; a restore asks *"do the bytes equal what I saved"*. **They
   coincide only in the clean case, and an agent who has just used numstat-zero correctly twice
   is the most likely to reach for it on the third.**

   ⛔⛔ **AND `cp -p` RESTORES THE *PRE-MUTATION MTIME*, SO A BYTE-IDENTICAL RESTORE CAN TEST
   RED. THIS IS THE FALSE-RED TWIN AND IT AFFECTS EVERY MANDATORY KNOWN-BAD LEG IN THIS REPO.**
   Found 2026-09-11 on `poumg.5` after a correct restore **tested RED twice**, and the agent
   proved the source was already correct on the worker with
   `rch exec --job -- sha256sum <file>` rather than assuming its own restore had failed.

   **The mechanism, verified locally:** `cp -p` copies the SOURCE's mtime onto the destination,
   so restoring from a `cp -p` aside-copy moves the file's mtime **BACKWARD** to before the
   mutation. The mutant `rlib` in the worker's `target/` was compiled *after* that timestamp, so
   **cargo's mtime fingerprint calls the source fresh and REUSES THE MUTANT ARTIFACT.** The test
   then runs the mutant against restored source.

   ```
   cp -p mt_a mt_b   -> mt_b takes mt_a's OLDER time     <- restore moves mtime BACKWARD
   cp    mt_a mt_b   -> mt_b takes NOW                   <- forward, cache correctly invalidated
   ```

   **`touch` fixed it with the content unchanged and `sha256` identical** — which is the proof
   that the RED was a staleness artifact and not the code.

   **THE TRAP IS THAT EVERY CORRECT INSTINCT POINTS THE WRONG WAY.** `sha256` matches, `cmp` is
   clean, `git status` is empty — every content oracle says the restore is perfect, and the
   suite is red anyway. The natural conclusion is *"my restore failed"* or *"the fix is wrong"*,
   and both are false. **Content oracles cannot see a build-cache key that is not content.**

   **HOUSE FORM, amended:** keep `cp -p` for the ASIDE copy (it preserves the original's
   metadata), but after restoring, **`touch` the file** — or restore with plain `cp`, which
   stamps NOW. Then re-run. **A RED that survives a `touch` is a real RED; a RED that clears
   under `touch` was never a measurement.**

   ⭐ **THE DIAGNOSTIC THAT ISOLATES IT, and it is the half that stops you reaching the wrong
   conclusion:** run `rch exec --job -- sha256sum <file>` **on the very worker that just went
   red**. If it returns the CORRECT hash, the source there is already right and the red is a
   STALE BUILD CACHE — not a failed sync, which is what an agent guesses first and which would
   send it re-transferring or re-editing correct code. **The remote content oracle separates the
   two; no local check can.**

   **AND THE RESTORE MUST STILL BE PROVEN BY `sha256` + `cmp` AGAINST THE ASIDE COPY, NEVER BY
   THE SUITE GOING GREEN** — the suite is precisely the instrument this defect corrupts, so
   using it as the restore oracle is circular.

   ⭐ **INDEPENDENTLY CORROBORATED THE SAME HOUR, DIFFERENT CRATE, DIFFERENT AGENT.** A second
   mutation lane hit it on `crates/s1-coverage/src/main.rs` — `cp -p` restore, sha256
   `7a3ed877…cba272` matching, `cmp` clean, `git status` empty, and the run still reported the
   previous mutant's failure. It caught the contradiction (*the mutation was gone from the
   source it was reading*), applied `touch`, and went green. **Two lanes, two crates, one
   evening: this is a property of the build path, not one crate's bad luck.**

   **Same family as rule 8 and as tonight's borrowed-green:** three different times in one
   session, the build system's view of "what changed" diverged from the tree's, and each time an
   agent nearly reported a property of the CACHE as a property of the CODE.

   **NO-CLAIM.** This makes a restore *correct*; it does not make a mutation *attributable*. A
   leg still has to show the RED was caused by the mutation and not by unrelated breakage — the
   discriminator is that the other legs stay GREEN, per rule 7.


8t. **AN ABSENCE CLAIM IS ONLY AS GOOD AS THE ARTIFACT YOU SEARCHED — KEY THE TABLE TO THE
   ARTIFACT, NOT THE NEEDLE.** Added 2026-09-10 after the conductor declared an error code
   nonexistent, retracted it, and then over-corrected into retiring a sound instrument. Converged
   from three agents in one hour.

   ```
   ARTIFACT   NEEDLE                                   ABSENCE MEANS
   SOURCE     any literal actually written there       SOUND      nothing composes it
   BINARY     a NON-COMPOSED string literal            USABLE     stored verbatim in .rodata
   BINARY     an identifier, release build             BLIND-ish  inlined / no retained symbol
   BINARY     format!-composed text or assembled code  BLIND      split around placeholders
   ANY        runtime observation                      BEATS ALL FOUR
   ```

   **THE AXIS IS "IS THIS EXACT BYTE SEQUENCE STORED CONTIGUOUSLY", NOT identifier-vs-message.**
   This rule was drafted with the middle two rows INVERTED and was corrected by measurement
   before it committed. On a release Mach-O with every needle confirmed present in source:

   ```
   parity_verdict               identifier   source 9   strings 0
   PARITY_REASON_CURRENT        identifier   source 3   strings 0
   probe_installed_verb_parity  identifier   source 1   strings 0
   missing_provenance           identifier   source 16  strings 3   <- ALSO a JSON key literal
   "installed verb set matches current source"   message    strings 1
   "this build cannot state its own origin"      message    strings 2
   zzz_absent_zzz               control                   strings 0
   ```

   Reproduced independently on a second release binary: `section_from_description` — a pure
   function name, never written as a literal — scores **0**, while `adopted_method` (7) and
   `OMPO_VERB_PARITY_CURRENT` (1) survive **because they are also string literals**. Identifiers
   are not merely weak in a release build, they are mostly GONE; non-composed message literals
   sit in the data section verbatim. **`RCH-E327` failed its sweep because it was COMPOSED, not
   because it was a message** — keying the rule to message-vs-identifier explains that one case
   by accident and gets the general population backwards, licensing exactly the inference that
   failed twice tonight: *"I grepped an identifier, got zero, therefore absent."* On a release
   binary that is wrong roughly three times in four.

   **CHEAP DIAGNOSTIC FOR THE COMPOSED ROW: grep TWO CLAUSES OF THE SAME MESSAGE.** If one hits
   and the other does not, you are reading fragments around `format!` holes and every absence in
   that neighbourhood is uninformative. Measured: `WRONG PLATFORM` → 1 while
   `the retrieved artifact(s) are ELF`, from the SAME sentence, → 0.

   ⛔ **AND THE POSITIVE CONTROL MUST COME FROM INSIDE THE ARTIFACT UNDER TEST, IN THE SAME
   CLASS.** A needle believed to be an identifier (`Selected_worker`) scored 0 and was
   **indistinguishable from the negative control**, which also scored 0 — and it was prose from
   a log line. But a class-check alone would NOT have saved it either, because the class itself
   is unreliable in that artifact. The only sound form is: **grep something you have confirmed
   is present IN THAT BINARY before believing any zero from it.** `missing_provenance` = 3 is
   what makes the surrounding zeros interpretable; without it there are four absences and no way
   to separate instrument failure from real absence.

   **`8i` DOES NOT CATCH THIS AND NEITHER DOES ITS TWO-PROBE CONTROL** — both establish that the
   INSTRUMENT can fire, not that your NEEDLE'S POPULATION is visible to it. That is a THIRD
   probe, and it must be drawn from inside the artifact under test.

   ⛔ **THE DEGENERATE CASE, AND IT IS WORSE THAN SKIPPING AN AVAILABLE CONTROL: SOMETIMES NO
   SAME-CLASS CONTROL CAN EXIST.** For `RCH-E327` there was none to find anywhere in the
   artifact — **no assembled code is contiguous, by construction**, so there was no second needle
   of that class in that binary at all. The control that WAS run, `grep -c 'RCH-E'` → 16, is
   right-artifact-WRONG-CLASS: a contiguous prefix literal standing in for an assembled code. It
   proved the instrument fires on a class the needle was never in.

   **When no same-class control CAN exist, the instrument is INAPPLICABLE and the honest verdict
   is `UNKNOWN`, not absence.** The other sightings that night each had a cheap missing arm; this
   one had none, which is a different and worse failure — there is no better grep, only a
   different oracle. A runtime observation settled it, and the registry above would have settled
   it sooner.

   ⛔ **AND IT GENERALISES BEYOND `grep`: ANY DIAGNOSTIC STRING YOU REASON FROM IS AN INSTRUMENT,
   AND MOST NAME A NARROWER CONDITION THAN THEY APPEAR TO.** Fourth sighting the same night, and
   the first in an error message rather than a search:

   ```
   Selected_worker                      prose read as an IDENTIFIER
   RCH-E327 absent from strings         an ASSEMBLED code read as NONEXISTENT
   "exists on disk, but not in '<rev>'" a WORKING-TREE hint read as PROOF THE REV RESOLVED
   ```

   The third cost the most and survived three agents. It was taken to mean *"the rev resolved but
   its tree lacks the path"*, and a whole object-divergence theory was built on it. **A bogus
   all-`deadbeef` sha produces the identical message** — so it means the rev did NOT resolve, and
   git is being helpful about your working tree. The resolvable-but-missing case says
   **`does not exist in '<rev>'`**, a different string that neither observation ever produced.
   The control was one run away and was explicitly advised against as not worth spending.
   **SO BEFORE YOU REASON FROM AN ERROR STRING, MANUFACTURE THE CONDITION IT SUPPOSEDLY RULES
   OUT AND CHECK THE MESSAGE CHANGES.** Not more evidence for the condition you believe — the
   OTHER one. **A message that cannot distinguish two states is not evidence about either.**
   That is `8i`'s two-probe control aimed at a diagnostic instead of a needle, and in all three
   sightings the missing arm was the cheap one: `Selected_worker` needed a confirmed-identifier
   arm, `RCH-E327` needed the runtime arm, `exists on disk` needed the unresolvable-rev arm.
   **One run each.**

   ⛔ **AND THE RUNG ABOVE ALL OF THIS: ASK WHETHER THE ARTIFACT SHIPS A REGISTRY FOR THE THING
   YOU ARE LOOKING FOR, BEFORE YOU SCAN ITS BYTES.** Four agents spent an hour deciding whether
   `RCH-E327` existed — `strings` sweeps over eleven binaries, a raw byte dump, a runtime
   observation — and the tool had shipped the oracle the whole time:

   ```
   rch error explain RCH-E327   ->  BuildArtifactForeignTarget, category + description + remediation
   rch error explain RCH-E300   ->  BuildCompilationFailed          <- positive control, resolves
   rch error explain RCH-E999   ->  Unknown code "RCH-E999".        <- negative control, REFUSES
   ```

   **A registry with a working negative control is a SOUND instrument** — it discriminates, which
   is the whole property a `strings` sweep lacks. `rch --help` advertises it. So the ordering is:
   **the tool's own oracle → a runtime observation → a scan of its bytes**, and the last is a
   last resort that cannot prove absence at all.

   **AND ASKING THE REGISTRY PAID A SECOND TIME, WITH THE FINDING THAT MATTERED.** Its
   remediation list names *"pin the build to an explicit `--target`"* — **the exact form this
   repo forbids**, because `--target` sets `required_os` and collapses the admissible worker
   fleet. Its other two remedies require a same-platform worker, which this fleet does not have.
   **The one workaround that works, `RCH_ALLOW_FOREIGN_ARTIFACTS=1`, is absent from the list.**
   No byte scan could have found that: the defect is not in the code, it is that **every
   documented remedy is unavailable or harmful here** — a strictly stronger report than "your
   guard has a bug", and it came from reading the tool's own documentation surface.

   **AND THE SCOPE OF A SWEEP IS PART OF ITS NEEDLE.** Measured the same night while counting
   beads whose DESCRIPTION carries an `ACCEPTANCE` heading the dispatcher cannot parse. A sweep
   that greps for that needle without scoping to the description ALSO matches every row whose
   populated `acceptance_criteria` FIELD merely begins with an `ACCEPTANCE`-ish line — rows that
   are perfectly dispatchable, because a populated field means the parser never reaches the
   description at all. That confounder was a substantial fraction of the headline figure. Two
   populations, one needle, and the larger one is the wrong answer. **Say which FIELD you
   searched, not just which string.**

   **AND "EXPOSURE" AND "VICTIMS" ARE TWO DENOMINATORS THAT BOTH SOUND LIKE THE ANSWER.** Same
   corpus, same hour: a large set of rows carry an unparseable `ACCEPTANCE` heading (**exposure**),
   while **victims = 0** because none of them ALSO has an empty field. A peer measured victims,
   reported it as *"trap count = 0"*, and had every byte needed to print both. **The reassuring
   figure was the one that got published.** Worse, the shape it named — the parenthetical — was
   a small minority; the dominant form was inline prose after `ACCEPTANCE:`. Fixating on the
   instance in front of you and then reporting a zero that is really about something else is how
   the larger population stays invisible. **Print both numbers, or name which one you mean.**

   **THE INTEGERS ARE DELIBERATELY NOT QUOTED HERE, AND THAT IS THIS RULE APPLIED TO ITSELF.**
   Two agents derived the exposure independently and got **243 of 720** and **256 of 743** —
   different definitions of "live row" and of "alone on its line", both defensible, neither "the"
   count. The SHAPE survives every denominator: **any inline text after the heading defeats
   `eq_ignore_ascii_case`.** The integer is true at exactly one denominator and decays like every
   other figure in this file.

   **THE MEASURED CASE.** `strings $(command -v rch) | grep -c 'E327'` → **0**, repeated across
   eleven binaries, and the conclusion *"the code does not exist"* was false. The message is one
   `format!` string stored in fragments around its placeholders — the stored bytes read
   `"O SUCCEEDED but returned executables for the WRONG PLATF"`, **starting mid-word** — so no
   contiguous literal holds the assembled text or the interpolated code. `RCH-E327` was then
   **observed verbatim in live output**, which settled it.

   **WIDENING A BLIND SWEEP DOES NOT MAKE IT SIGHTED. Eleven blind reads are still blind.**

   **AND THE POSITIVE CONTROL WAS AVAILABLE AND UNRUN**: `grep -oE 'RCH-E[0-9]{3}' | sort -u` →
   **27 distinct codes**, so the instrument demonstrably CAN return a code and `E327`'s absence
   from that list is evidence of nothing. *(Denominator: 27 distinct CODES; `grep -c 'RCH-E'`
   → 20 counts LINES. Quote the 27.)*

   **DO NOT RETIRE THE SOUND ROW.** The first over-correction here read as retiring `grep` for
   absence wholesale. A bead had closed on greps for `IoFailed` / `PipeReadOutcome` /
   `terminate_and_reap` returning 0 — **over SOURCE**, the top row, independently confirmed by
   `cargo check --all-targets` exiting 0. That verdict was never exposed to the defect at all,
   because the defect is a binary property. A blanket *"grep cannot prove absence"* would have
   cast doubt on a verdict the hazard could not reach. Same shape as `8s`: **a rule stated too
   broadly forbids a legal operation.**

   **NO-CLAIM.** SOUND here means only that the needle cannot be hidden by COMPOSITION. It does
   not mean your needle was right, your corpus complete, or your tree the one you meant — `8i`'s
   two-probe control still applies on top of this, and the `Selected_worker` case above is an
   instance where `8i` alone would also have passed.

8q. **AN `OR`ed READBACK NEEDLE IS ONLY AS STRONG AS ITS WEAKEST ALTERNATIVE — it confirms the FILE,
   not the EDIT.** Measured 2026-09-07 by `%20`, which caught it because two instruments disagreed.

   ```
   grep -cE 'cannot read STAGED blob|is not UTF-8'  at HEAD  ->  2   reads as "my change landed"
   staged_blob(&repo_root, staged_file)             at HEAD  ->  0   the change: ABSENT
   cannot read STAGED blob                          at HEAD  ->  0   mine: absent
   is not UTF-8                                     at HEAD  ->  2   PRE-EXISTING, another gate
   ```

   **Rule `8p` requires a `git show HEAD:<path>` readback. This is how that readback lies:** an
   alternation matches pre-existing text, so the readback passes while the edit is nowhere in the
   tree. **Use a needle UNIQUE to the change** — a new function name, a new literal — never a
   disjunction of plausible strings.

   **The tell is two instruments disagreeing**, which is why `%20` caught it and a single grep would
   not have. **Same family as `8o`:** a text count answering a different question than the one asked.

8p. **EVERY DISPATCH REQUIRES A CALLBACK, BECAUSE A WORKER HAS NO WAKE TRIGGER.** Joshua's call,
   2026-09-07, fleet-wide. Landed in the dispatch template and synced to
   `~/.config/ntm/templates/dispatch.md`, **which had been stale since Jul 26.**

   **This is a defect in the ARCHITECTURE, not in any pane.** A worker's turn ends and it waits for
   input. It cannot self-dispatch and nothing polls it. So the controller learns a pane is free
   **only if the pane says so** — and a pane that finishes silently is **indistinguishable from a
   pane still working.** The fleet then stalls on that ambiguity rather than on the work.

   **Measured that night: three panes went idle holding a FINISHED unit, each costing a round trip to
   discover.** The instruction previously given to one of them — *"self-dispatch"* — is something the
   architecture forbids. The callback is the correct fix, and it lives on the worker's side because
   only the worker knows when its turn is ending.

   **Fire it on all three outcomes:**

   ```
   DONE          the unit landed. Carries the sha AND its readback.
   BLOCKED       HIGHEST VALUE and the most skipped. The controller cannot route around a
                 blocker it does not know exists.
   NEEDS-RULING  a judgement that is not yours. Name the decision in ONE sentence.
                 Do not idle waiting on it and do not guess.
   ```

   **A `BLOCKED` or `NEEDS-RULING` callback is a SUCCESS, not a failure report.** A worker reporting
   `NO_ELIGIBLE_TARGET` with evidence has **succeeded** — that is a signal about the QUEUE, not about
   the worker.

   **What every callback carries:**

   - the bead id or row touched
   - the commit sha **and** its `git show HEAD:<path>` verification. **A bare sha is not proof the
     content survived; `1 file changed` is the command succeeding, not the content persisting.**
   - for ANY remote build, **both** proof lines — `Remote command finished: exit=<N>` **and**
     `test result:`. **A refused `rch` build EXITS 0, so their ABSENCE is the tell.**
   - `NEXT:` what you would pick up unprompted. **This is what turns two round trips into one.**
   - `NO-CLAIM:` the precise limit of what you proved.

   **DO NOT BATCH.** Report when the unit lands, not at the end of several — the point is that the
   controller learns you are free **at the moment you become free.**

   **The dispatcher's half:** a packet that does not state this contract owns the silence it gets.
   Measured 2026-09-02 as a clean natural experiment — three packets carrying a stated bar produced
   three conforming contracts; the one packet that omitted it produced the only non-conforming
   deliverable, despite having the richest substance of the four.

8u. ⛔ **A `BLOCKING` SUBAGENT RUNS IN-BAND, SO EVERY INTERRUPT KILLS IT. CHECK THE AGENT'S
   BLOCKING FLAG BEFORE YOU SPAWN LONG WORK.** Added 2026-09-11. **(Filed as `8r` on first
   write, colliding with the parse-oracle rule at `:2006` — renumbered the same session by its
   author. A duplicate rule id makes every later citation ambiguous, and this file is cited by
   id constantly.)** Measured: **five subagents aborted at ~4 minutes each**, in two waves, and
   the conductor diagnosed it as *"background agents cannot
   survive the tick cadence"* — **which is false and would have cost the fleet half its
   remaining capacity**, on a night when the Codex panes were out of tokens and background
   agents were one of only two working vehicles.

   The agent list marks some types `(BLOCKING: inline result)`. Those execute **inside the
   conductor's own turn**, so the 5-minute orchestrator tick — or any user message — terminates
   them mid-flight. A non-blocking type (`task`, `scout`, …) spawns a real background job that
   **survives ticks and auto-delivers**. Re-spawned as `task`, the identical units ran straight
   through the next tick.

   ```
   zeststream-builder   BLOCKING -> aborted at 3m40s, 3m40s, 3m40s, 4m11s, 4m11s
   task                 background -> still running across the next tick
   ```

   **THE GENERAL DEFECT IS THE ONE THIS FILE KEEPS RECORDING: A PROPERTY OF THE INSTRUMENT READ
   AS A PROPERTY OF THE WORLD.** *"Background agents die here"* and *"I chose an inline agent
   type"* produce the identical observation and have opposite remedies — the first says stop
   parallelising, the second says pass a different string. Same family as `8i`: before
   concluding a capability is unavailable, **check the tool's own declaration of what it does.**
   `rch exec --job` was found the same night by the same question.

   **AND AN ABORTED SUBAGENT'S WORK IS NOT AUTOMATICALLY LOST — GO LOOK.** Of the five, one had
   already written a correct fix into the worktree and was killed before committing; it was
   verified remotely and landed as `063e67f`. Another had reached a real verdict (*"the named
   test passes but a SIBLING fails, and that inversion is the tell"*) that was carried into the
   re-spawn's packet. **Check `git status --porcelain` and the transcript before re-dispatching,
   or you pay for the same work twice** — and check it for DAMAGE too: a different aborted agent
   had left a regression plus a `100644 -> 100755` mode flip.

8v. ⛔ **`cp` OVER A LIVE MACH-O SIGKILLS IT, AND RESTORING BYTE-IDENTICAL BYTES DOES NOT FIX IT
   — macOS CACHES THE SIGNATURE PER *INODE*, NOT PER CONTENT. INSTALL VIA A FRESH INODE.**
   Measured 2026-09-11 by the conductor, who broke `ompo` on `PATH` for three minutes doing the
   install it had just authorised.

   ```
   cp certified -> ~/.local/bin/ompo      ompo health  rc=137   (128+9 = SIGKILL, empty output)
   cp -p backup -> ~/.local/bin/ompo      ompo health  rc=137   <- BYTE-IDENTICAL ROLLBACK, STILL DEAD
   ./grade-dist-good/bin/ompo-good        health       rc=0 GREEN  <- same bytes, untouched inode
   cp -> ompo.staged (new name)           health       rc=0 GREEN  <- fresh inode, runs
   mv -f ompo.staged ompo                 health       rc=0 GREEN  <- atomic rename, installed
   ```

   **THE ROLLBACK FAILING IS THE DISCRIMINATOR AND IT IS THE WHOLE LESSON.** A byte-identical
   restore that stays dead **proves the kernel's objection is not to the content**. Both
   binaries are `adhoc, linker-signed`; rewriting the pages of a running-or-cached inode
   invalidates AMFI's cached verdict for that inode **permanently**, so every later exec of that
   path dies regardless of what you put there. **Reaching for a bigger hammer on the bytes —
   re-copy, re-`chmod`, re-download — cannot work, and will read as "the new binary is broken."**

   **HOUSE FORM:** stage under a DIFFERENT NAME **in the destination directory**, verify it
   there, then `mv -f` into place. The rename gives the target path a fresh inode with an
   unpoisoned signature, and it is atomic so no window exists where the path is absent.
   **`dcg` refuses `mv` from a repo into `$HOME`** (`core.filesystem:mv-sensitive-source-root-home`)
   — `cd` to the destination directory and stage there.

   **AND `rc=137` IS NOT AN APPLICATION EXIT CODE.** `137`, `139`, `134` are `128+signal`; an
   empty stdout beside them means the process never ran its own code. **Do not read a signal as
   a verdict** — same family as the restrictive-terminal rule, where a timeout is not a verdict.

   **NO-CLAIM.** This is the install path on macOS with adhoc-signed cross-built binaries. It
   says nothing about notarized artifacts, and nothing about why the signature was cached — only
   that a fresh inode clears it, measured five ways above.

9. **NO ACCEPTANCE IS COMPLETE WITHOUT A WIRING-PROOF LEG. The dispatch is where BUILT ≠ WIRED
   gets in.** Measured 2026-09-06, and it is the orchestrator's own defect: every acceptance
   written that session demanded fires-on-known-bad, a known-good leg, a mutation leg and
   anti-vacuity — and **not one demanded a caller.** So `crates/blocker-taxonomy` shipped at 662
   LOC with 19 green tests, satisfied its acceptance completely, and was invoked by nothing:
   `grep -rl blocker-taxonomy crates/*/Cargo.toml` minus its own manifest -> **0**;
   `blocker_taxonomy` references outside its own crate -> **0**. The acceptance never asked, so
   the pane never wired it, and the pane was right.

   **Every acceptance for a new mechanism MUST carry this leg, stated as a command:**

   ```
   WIRING PROOF: after this lands, both of these are NONZERO and pasted in the close reason —
     grep -rl "<crate>"  crates/*/Cargo.toml | grep -v "crates/<crate>/Cargo.toml" | wc -l
     grep -rl "<crate_underscored>" crates/*/src | grep -v "crates/<crate>/src" | wc -l
   or the lane is declared unwired with a named row and a reason. Silence is not an exception.
   ```
   **AND THE PROOF MUST TERMINATE AT A REACHABLE TRIGGER, NOT AT ANOTHER UNCALLED CRATE.**
   Measured 2026-09-07, roughly one hour after this rule landed, and the rule as first written
   permitted it. Bead `nxc9` asked for a consumer of `dispatch_saga`'s routing decision. `%9`
   delivered `crates/m2-grading-lane`, which depends on `dispatch-saga` and calls
   `dispatch_saga::m2::route`. The leg passed exactly as specified — `dispatch-saga` went from 1
   caller to 2. Then:

   ```
   m2-grading-lane external callers                          0
   m2-grading-lane bin                                       1
   m2-grading-lane refs in workflow/hook/.flywheel/cron      0
   ```

   **The uncalled-ness moved up one level.** `%9` did precisely what the acceptance asked and the
   acceptance was mine, so this is the orchestrator's defect for the second time in one hour: a
   wiring proof that terminates at a fresh consumer proves only that *two* crates are now unwired
   instead of one. It is a shell game the rule cannot see, because each individual grep is honest.

   **The leg is therefore two-part, and the second part is rule 3:**

   ```
   WIRING PROOF, part 2 — walk the caller chain to its ROOT and name what fires it:
     for each crate in the chain, external callers; when a crate has none, it MUST have a
     reachable trigger — a .github/workflows entry, a .git/hooks script, a .flywheel/ file,
     a crontab or launchd row, or a Command::new spawn from a crate that itself has one.
   A chain ending in a crate with neither a caller nor a trigger is UNWIRED, however many
   manifest edges were added along the way.
   ```

   The cheap check is one command per link, and the chain is short by construction — if it is long,
   that is itself the finding.


   **RETRACTED, ONE HOUR AFTER I PUBLISHED IT. The first version of this paragraph read: "36 of 37
   TRIGGERED and exactly 1 untriggered… BUILT ≠ WIRED here is not a standing swamp; it is a
   NEW-crate window." THAT IS FALSE AND IT WAS THE WORST ERROR OF THE SESSION, because it went
   into this file where every agent reads it as doctrine.**

   The defect: I counted a mention in a `.flywheel/` markdown file as a *trigger*. **A document
   that names a binary does not run it.** `.flywheel/AUTONOMOUS-WAVE.md` naming `fast-dispatch` is
   prose, not an executor. Re-measured 2026-09-07 with comments stripped from every surface and
   with executors separated from documents:

   |of 36 bin crates with zero library callers|count|
   |---|---:|
   |**EXECUTOR** trigger — `.github/workflows`, `.git/hooks`, crontab, launchd, or a `Command::new` spawn|**12**|
   |**DOCUMENT** mention only — `.flywheel/` prose or registry|**23**|
   |neither|**1**|

   **So 24 of 36 have no executor trigger, not 1.** Wrong by a factor of 24, and in the reassuring
   direction — which is the direction that stops people looking.

   **And the doc-only set is the worst possible list**, because it is largely the kernels this file
   orders agents to use instead of handrolling: `fast-dispatch`, `fleet-monitor`, `fleet-truth`,
   `loop-driver`, `refill-idle-panes`, `omp-idle-dispatch`, `bead-availability`,
   `kernel-only-operator-hook`, `s1-coverage`, `s2-gate`, `crate-soundness-verify`,
   `oracle-pane-state-differential`, `pane-oracle-diff`, `inbox-moni`… twenty-three of them. The
   KERNEL-ONLY section below asserts these are *"all installed and cron-scheduled."* **They are
   not.** The only crontab line naming any of them is a **comment** — an epitaph reading
   *"refill-idle-panes refused 51 ticks, 2026-09-02"* — so the rows were removed and the claim went
   stale with them.

   **BUILT ≠ WIRED here IS a standing swamp.** Two thirds of our bin kernels have no executor. The
   corrected shape is worse than the original claim in scale and identical in mechanism: nobody
   noticed because each individual grep was honest and the coarse signal was flattering.

   The reusable rule, which is the third time this class has bitten in one session: **`grep`ping a
   directory tree for a binary's name measures MENTIONS, and a mention is not an invocation.**
   Separate executors from prose before you count, and strip comments from every surface including
   `crontab -l`, not just the ones you remembered.

10. **A RATCHET KEYED ON AN ABSOLUTE COUNT CANNOT TELL GROWTH FROM REGRESSION.** `crate-atom-gate`
   is the gate for rule 9 and it is correct: 1701 LOC, nine parts, `UNWIRED_ALLOWANCE: &[] = &[]`
   exactly as this file prescribes, systemic allowances carrying `owner=` and `dies_when=`, and
   `check --repo .` scans **84 crates / 747 rows** and exits **1 / `ATOM_INCOMPLETE`**. It would
   have caught the crate above.

   **It has never been invoked.** References to it in any workflow, hook, `.flywheel/` file or
   crontab: **0**. So the gate that enforces "nothing ships unwired" is itself the purest instance
   of the thing it forbids — rule 3 aimed at the rule-3 enforcer. The bead that owns wiring it
   (`omp-orchestrator-d3gm`, P0) has been `in_progress` and stalled for the whole session.

   And wiring it as-is would refuse forever, for the wrong reason. Its ceilings were set at a
   ~69-crate workspace; the workspace is now 83, and three parts move in **exact lockstep** with
   that growth:

   |part|live|ceiling|gap|
   |---|---:|---:|---:|
   |part4 tests|82|68|**14**|
   |part6 claim|83|69|**14**|
   |part8 oracle|83|69|**14**|
   |part9 wired-caller|29|26|3|
   |part1 lib|1|2|**slack**|

   **14 = the number of crates added.** Every new crate adds a row to every part it does not
   satisfy, so a compliant addition breaches the ceiling identically to a broken one. Read
   correctly, `part9` **improved**: 11 of the 14 new crates DID get a caller. Read by the gate, it
   is a `CEILING_BREACHED`.

   This is the same family as the docs-staleness metric in the post-mortem below, which re-stales
   `AGENTS.md` in ~25 minutes during a wave: **a counter that grows with healthy activity is red
   precisely when the fleet is most productive**, and a gate that is red by construction gets
   routed around. Express the ratchet as a per-crate assertion or a ratio, never a workspace-wide
   absolute, and only then wire it fail-closed.

## Instrument contracts: what each surface ACTUALLY returns

**Receipt:** [`CENSUS-ARCHIVE-INSTRUMENTS.md`](docs/inventories/CENSUS-ARCHIVE-INSTRUMENTS.md#instrument-contracts-what-each-surface-actually-returns)
— a table of **measured surprises, not a specification.** Its counts are archived and **not
citable**; re-run the producing command.

**The row that is still live and costing work: an agent NAME is not an identity.** `WildStone`
carries four bindings — bare (36 beads), `pane=%8` (26), `pane=%7` (8), `pane=%9` (5, a dead pane) —
so a claim or reservation held by a bare name **cannot be attributed to a worker.** Claim as
`pane=%N;agent=NAME`. Tracked as `omp-orchestrator-ptkmi`.

---

## A TRANSCRIBED VALUE IS STALE BY DESIGN

**Receipt:** [`LIFECYCLE-FAILURES.md#a-transcribed-value-is-stale-by-design--five-substrates-one-shape`](docs/negative-patterns/LIFECYCLE-FAILURES.md#a-transcribed-value-is-stale-by-design--five-substrates-one-shape)
— five instances, five substrates, one defect.

**Bind a claim to something that RE-DERIVES the value, or carry its measurement time and expiry.
Never transcribe a value you own.** And **carry the population and the TREE with every figure** — a
bare `cargo test` aggregate is inadmissible, because `cargo` reads the worktree while a sha names a
tree (rule 8).

### AND A BEAD'S ACCEPTANCE IS A TRANSCRIBED VALUE — RE-DERIVE IT BEFORE YOU DISPATCH IT

**Binding on the DISPATCH path. Measured 2026-09-08: 170 of 755 non-terminal beads cite a hard
count, and every one checked that night was stale — three of three.**

```
n4q   "6 unbounded .output() spawns"      -> historically 5, currently 0. Fix landed in 2b6b2d7.
i0uh  "12 records lack purpose/inputs/…"  -> all 12 already carried every field.
815   "14 crates are TERMINAL"            -> the audit derived 1. Thirteen misclassified,
                                             ONE OF THEM A LIVE CRONTAB EXECUTOR.
```

**Every one was stale in the direction that makes a bead look like live work**, so the queue serves
it forever and a pane spends its unit discovering the premise is gone. **`815`'s version nearly cost
a deletion of a running crate.**

**THE RULE: before a packet is sent, re-derive any count, path, or state the acceptance asserts.
If it moved, amend the acceptance FIRST — never after the send**, because an acceptance edited
mid-flight moves the target under a worker (see the dispatch-only-instruction rule above).

**And the verdict a stale premise earns is not `WONTFIX`:**

```
ALREADY-FIXED   the defect is gone. Close with the sha that fixed it and a NON-AUTHOR grade.
PREMISE-FALSE   the count/state was never right. Amend the acceptance with the measurement.
STILL-LIVE      re-derived and unchanged. Dispatch it.
```

**A dispatch against a stale premise is indistinguishable from a dispatch against a real defect
until the pane reports back** — which is a full unit spent to learn nothing, and it is the
dispatcher's cost, not the worker's.

### AND AN `ACCEPTANCE` HEADING IN A DESCRIPTION MUST BE ALONE ON ITS LINE

**Measured 2026-09-10 with `dispatch_packet::render` itself as the oracle**, run over every live
tracker row rather than a hand-rolled needle. The dispatcher's fallback
(`dispatch_packet.rs:172-175` → `section_from_description`) matches the **whole line**,
case-insensitively, after stripping leading `#` and **at most one** trailing `:`. **So any inline
text after the heading defeats it.** All three of these are invisible to the parser while reading
as headings to every human and matching every substring grep:

```
ACCEPTANCE: <criteria>                 <- the DOMINANT shape in this tracker
ACCEPTANCE (run X, expect Y):          <- repeated boilerplate across many beads
ACCEPTANCE — one of these two, not both
```

**Put the heading ALONE on its line, exactly `ACCEPTANCE`, with at most one trailing colon, and
start the criteria on the NEXT line.**

**This costs nothing while the `acceptance_criteria` FIELD is populated** — a populated field
means the parser never reaches the description, and measured over the live corpus **zero rows
currently reach the fallback at all, so it is dead code today.** It bites at exactly one moment:
when an author leaves the field empty and trusts the heading. The bead then **silently stops
being dispatchable** while looking complete to every reader and every grep.

**NO-CLAIM.** This is authoring guidance, not a gate — nothing refuses a malformed heading, and
the exposure is counterfactual rather than a live defect. Deliberately NOT filed as a bead: the
victim count is zero, and filing for a zero is the ceremony this file forbids.

### ⛔ AND A CONDUCTOR'S *RULING* IS A TRANSCRIBED VALUE TOO — IT CAN BE STALE BEFORE IT IS DELIVERED

**Measured twice on 2026-09-11, both by pane 1, both from a snapshot of shared mutable state.**

**Instance 1:** named a superseded CI run *"authoritative"* in the tick block — **twice in two
hours, the second time by the agent that had just fixed the first.**

**Instance 2, the sharper one:** saw ` M` on `resident.rs` and `select.rs`, inferred a
collision between two live agents, and issued an ownership ruling **without asking who held
what.** They had already exchanged exact line ranges and agreed a four-edit split —
**unprompted, and before the ruling existed.** The conductor had told them to coordinate
directly and then stepped on the coordination that had already happened.

⛔ **`git status` CAN TELL YOU A FILE IS DIRTY. IT CANNOT TELL YOU THE DIRT IS NEGOTIATED.**
A conductor is a round trip away from the thing it rules on, so **between observation and
delivery the subject moves** — which is why live peer coordination beats routing through the
conductor, and why `hub`'s own advice to have siblings message each other is not a nicety.

**THE RECEIVER'S HALF, and it is the part that saved the work: "I AM FREEZING, NOT REVERTING."**
The held agent had already landed six sites. It stated the facts, named exactly what it would
and would not do, and **declined to undo landed work to satisfy a stale order** — because
reverting would have caused the very clobber the ruling existed to prevent. **That is the
correct response to an instruction premised on a state that has moved**, and it is reasoning
rather than obedience.

**THE MECHANICAL FORM:** before ruling on who owns a file, **ASK**. Peers know their own
reservations; `git status` knows only bytes. And when a ruling arrives that contradicts what you
have already landed, **report the divergence and freeze — never revert to comply.** A stale
order executed faithfully destroys more than a stale order refused.

---

## A BORROWED CLAIM INHERITS ITS AUTHOR'S BURDEN

**Receipt:** [`LIFECYCLE-FAILURES.md#a-denied-or-errored-probe-is-unknown-never-a-negative-result`](docs/negative-patterns/LIFECYCLE-FAILURES.md#a-denied-or-errored-probe-is-unknown-never-a-negative-result)
— including the measured case where a `dcg`-denied probe became an asserted negative, then house
doctrine, across three agents in under an hour.

**Restating someone else's finding makes it yours.** Cite it and verify it, or attribute it and mark
it unverified. **There is no third option in which you hold it as fact because a peer measured it.**

**The measurable tell: if you can state a claim's file and line but have not opened that file, you
are transmitting, not verifying.** A peer's claim arrives with the social weight of collaboration
rather than the suspicion we reserve for our own probes, which is why this is easier to forget than
`8i`.

**And a retraction is not a licence to publish the next plausible story** — the replacement needs the
same standard as the thing it replaces.

## Before calling anything a defect, READ THE DEFINITION OF CORRECT BEHAVIOUR

**A reproducible observation plus a plausible story is not a defect. It is a hypothesis — and
upstream usually has a comment about it.**

Measured 2026-09-02. One agent produced three diagnosis errors in a single session and they were
all the same shape: **every measurement held up; every diagnosis was made without first reading the
code that defines correct behaviour.** In all three cases the "defect" turned out to be specified,
deliberate, or documented — twice with a source comment explaining precisely why the reading was
wrong.

|"defect"|what it actually was|
|---|---|
|`mail_pending` — "the flag is the discriminator", then "unbounded"|a documented **300s** internal deadline. Every `CANCELED` was the caller's own SIGTERM landing first, and `waited_seconds` tracked the caller's ceiling linearly at 75/90/180. Only a **340s** ceiling could discriminate|
|`signaled=false` with `persisted=true, acknowledged=true` — "the cleanest silent-notification defect of the night"|specified debounce. `messaging.rs:4259` is `"signaled": !signal_receipts.is_empty()`; `:4219` documents the state; `sync.rs:134-138` states the mutable `.signal` file is intentionally not read|
|the `CURSOR_EXPIRED` "silent clamp" — broadcast as the purest silent-success instance found|**the deliberate fix for GH#238.** `sync.rs:606-624` at 0.3.32 explains that a gap between `after` and a recipient's oldest is *other recipients' deliveries, not lost history*, and that a monitor calling `--position-now` on an empty inbox **must still receive its first delivery**. `retention_has_pruned = global_oldest_cursor > 1`|

**THE THIRD ONE IS THE CAUTIONARY TALE, because the "defense" reproduced the bug upstream had
fixed.** A client-side guard (`verify_resume_continuity`) refused whenever
`oldest_available > stored` — which refuses resuming from origin for EVERY recipient, since
`oldest_available_cursor` is `MIN(seq) WHERE project_id AND agent_id`, a **first-event marker**, not
an eviction floor. Worse, the client *cannot* make that judgement: the response exposes
`oldest_available_cursor` and `tail_cursor` but **not the global oldest**, so `retention_has_pruned`
is unknowable client-side. The guard decided expiry with strictly less information than the daemon
has, and got it backwards. **The fix was a deletion**, and the correct posture is to trust the
daemon's typed refusal and decode it — never to synthesise one locally.

**Two operational consequences:**
- **Version-match before diagnosing.** Reading source at the *shipped* version turned all three
  arguments into one-minute reads. Our working checkout was 1,285 commits behind while the mirror
  was at 0.3.32 — *ahead* of the shipped binary. A defect claim against source you do not hold is
  not a claim.
- **A guard is a claim too, and it inherits this rule.** Shipping a defense against a
  misdiagnosed defect is worse than shipping nothing: it refuses healthy traffic, it is advertised
  as protection, and it is harder to retract than a note. Before writing a guard, read what the
  thing you are guarding against is *supposed* to do.

---

## Three arbitrations from a five-agent shared checkout

**1. A RESERVATION ANSWERS "MAY I EDIT NOW". AN OWNERSHIP MAP ANSWERS "WHOSE LANE IS THIS". A green
reservation is NOT a transfer of ownership.** Measured 2026-09-02: `.flywheel/AUTONOMOUS-WAVE.md:59`
assigns `%1414` the binary — `crates/omp-orchestrator/**` — while an Agent Mail exclusive
reservation on `crates/omp-orchestrator/src/main.rs` returned `conflict_free: true`,
`total_conflicting_reservations: 0`. Both readings were correct and they answer different questions:
the reservation system is the only surface that can **REFUSE at edit time**, so it governs
concurrent safety; the map expresses **intent**, so it governs whose lane a change belongs to. A
`conflict_free` green means only that nobody else holds a lease *right now* — it does not mean the
path is unowned, and an owner who never took a lease is invisible to it.
  **RULING:** the reservation governs the edit, the map governs the assignment, and neither
  substitutes for the other. Cross-lane edits are legitimate when the orchestrator assigns them
  explicitly — but the assignee MUST disclose the crossing, as this one did, and the lane owner
  keeps the follow-on work. Treating a green reservation as an ownership claim is the same category
  error as treating a stale roster table as a live handle.

**2. A KNOWN-BAD FIXTURE MUST NOT LIVE UNDER `crates/`.** The root manifest uses
`members = ["crates/*"]` **deliberately**, so no pane holds a reservation on the root `Cargo.toml`.
Its one sharp edge: a directory with a `Cargo.toml` and no `src/` breaks workspace **LOADING**, so
`-p <your-crate>` cannot dodge it and EVERY cargo command in the repo fails. This happened **twice
in one hour** — `crates/response-envelope-check` at 23:29, `crates/zz-planted-dup` at 05:57 — both
from gate authors planting fixtures, both self-resolved within minutes, and both read to other
agents as "every cargo command is failing".
  **RULING:** plant known-bad fixtures OUTSIDE `crates/`, or write `src/` and `Cargo.toml` in the
  same step. `Cargo.toml`-before-`src/` under a glob member is a fleet-wide stall, and a gate author
  is precisely the agent most likely to create one.

**3. `mtime` PROVES ORDERING, NOT PROVENANCE — and a sha diff between two differently-produced
artifacts is not a staleness test.** Measured: the installed hook `a87917e68…` and a freshly built
`pre-commit-gate` `9ca721b19…` have DIFFERENT shas while
`cargo test -p no-shell-gate --test hook_freshness` reports **3 passed / 0 failed**. Both facts are
true. Rust builds are not bit-reproducible by default, so two shas differing does not imply the
sources differ; and `hook_freshness` compares **mtime** (16 sites) — source-newer-than-hook — which
is an ORDERING check.
  **RULING:** the freshness oracle wins over an ad-hoc sha comparison, because it measures the
  question asked. But the instinct behind the sha check is right and names a real residual: mtime
  proves the hook is not OLDER than its source, NOT that it was BUILT FROM that source. A `touch`,
  or a rebuild with different flags, satisfies mtime while carrying different logic. Closing that
  needs the hook to embed and report its own source hash — a content-identity check, not a
  timestamp one. Until then, "the hook is fresh" is a floor, not a proof.

---

## Every DOCUMENT proves it bites, too

A contract is a deliverable with a pass bar, not prose. The bar and its runner live in
`docs/contracts/asupersync_process_grade.md`, derived from 97 asupersync contracts; the skeleton is
`~/.claude/skills/project-startup/assets/contract-template.md`.

| marker | corpus | why it is not optional |
|---|---:|---|
| `## Validation` — ONE pasteable command | 43% | without it, "is this still true" costs a grading ROUND |
| `Bead:` line | 40% | the doc↔DAG link; absent, the document is commentary and no bead can close on it |
| `## Contract Artifacts` incl. an **invariant suite** | 26% | a contract naming no test file is a DESCRIPTION |
| `## Purpose`, `## Cross-References`, ≥5 stable IDs | 47 / 38 / 50% | |
| ≤25 KB (corpus median **7,132 B**) | — | past that it cannot be verified in one sitting, so it gets graded in rounds — and a round finds drift, never absence |

**Measured 2026-09-02, and it is a clean natural experiment.** Four Phase 0 contracts were
dispatched. Three packets carried this bar plus the template path; one — written before the bar
existed — asked only for "carrier set, operations, laws, NON-COVERAGE". **The three conforming
packets produced three PASSING contracts; the under-specified packet produced the one contract that
failed all five markers**, despite having the richest substance of the four.

**Packet quality determined output quality with zero exceptions.** A dispatcher who omits the bar
owns the non-conforming doc, not the pane. State the bar in every packet, and have the pane run the
grader and paste its output before committing.

---

## Working here

**Beads.** This repo has its own `.beads` (prefix `omp-orchestrator`). `br` reads the cwd — **cd
first**. Never file substrate work in control-plane's tracker.

**Verification** (`/beads-compliance-and-completion-verification`): status is a **claim**, not a
fact. **Re-run, don't read** — a test that passed in CI yesterday is inadmissible. A test passes
meaningfully only when it (a) exists, (b) exits 0, **and** (c) asserts non-trivially against the
production path. Most theater is (c).

**Never silence stderr** in a command whose output you will cite as evidence.

**Search before building.** `mcp__socraticode__codebase_search` by meaning, then `fh suggest`.
`grep` is the follow-up that jumps to a line, never the opening move.

### USE THE STRUCTURAL TOOLS WHILE BUILDING CRATES, NOT AFTER (binding, Joshua 2026-09-07)

**Every tool below was verified present on this machine before being written here.** A prescribed
tool that is absent is worse than none — it produces a confident zero.

|question|tool|NOT this|
|---|---|---|
|who calls this symbol|`ripwire --callers=SYM` / `--uses=SYM`|`grep -c` — see `8o`|
|what does this symbol reach|`ripwire --callees=SYM`, `--impact=SYM`|reading imports|
|is this code dead|`ripwire --dead-code`, or `rustc`'s own warning|absence of grep hits|
|where is a symbol defined|`ripwire --whereis=SYM`|filename guessing|
|rewrite a pattern across files|`ast-grep` / `sg`|`sed` on source|
|workspace topology, crate counts|`cargo metadata --format-version 1` + `jq`|`grep`/`find` — see the crates section|
|does this markdown anchor resolve|`pandoc -f gfm -t html` and read the real `id=`|deriving a slug|
|has this been solved already|`fh suggest`, then `fh why <row>`|building it again|
|what should be worked next|`bv --robot-triage`|`br ready --json` by hand|

**FOUR MEASURED FAILURES FROM ONE SESSION, each of which a tool above would have caught:**

1. **`grep -c ompo` reported the flagship binary wired in 62 places. The true count is 0** — every
   hit was an `omp-orchestrator` **substring**. Caught only by grepping a binary known to be in the
   crontab as a control. **`ompo` is invoked by nothing: crontab 0, hooks 0, `.flywheel` 0, other
   crates 0, `Command::new("ompo")` 0.**
2. **`grep -c workflow_invokes_lint` returned 3 and the ruling built on it was wrong** — both call
   sites were **inside the function being deleted**. `rustc`'s dead-code warning settled it (`8o`).
3. **A shell census reported 1 workspace leaf where `cargo metadata` reports 33**, because
   `grep -c … || echo 0` emits `"0\n0"`.
4. **Two markdown anchors were derived wrong** — an underscore dropped and a triple hyphen missed —
   and would have shipped broken. `pandoc` rendering the real `id=` caught both.

**AND THE TOOLS HAVE THEIR OWN BLIND SPOTS, so pair them:** `ripwire --uses` returned **0** for a
symbol with two live call sites because they sat inside `assert!()` — the **macro-argument blind
spot** (`8k`). A structural zero from a macro-blind tool is `UNKNOWN`, not absence; `--grep` is the
prescribed follow-up. **`ripwire` emits ONE line**, so any line filter deletes the whole payload —
read raw and pass `--legend=compact`.

**THE RULE, one sentence:** for any question about *who calls*, *what reaches*, *what is dead*, or
*how many crates*, **a text count is not an answer** — use the structural tool, and when it returns
zero, prove the instrument can return nonzero before you believe it (`8i`).

**Commits.** Path-scoped with an explicit list — `git commit -- <paths>`. Never `-A`; a shared
checkout means a bare commit sweeps in another agent's unfinished work. Commit messages carry a
verification-level tag (`[test]`, `(code-first, test pending)`, `[selftest-verified]`).

**A path-scoped `add` does NOT make a bare `commit` path-scoped.** Measured 2026-09-02, twice in
consecutive commits: the index is **shared state**. `git add -- <paths>` followed by
`git commit -F <msg>` with no pathspec commits the ENTIRE INDEX, including whatever another agent
had already staged. Two commits swept a peer's 220-line taxonomy, 236 changed bead lines, and a
third agent's file rename, all under unrelated subjects.

```bash
git add -- <paths> && git commit -- <paths>   # correct — BOTH pathspecs
git add -- <paths> && git commit              # WRONG — takes the whole index
git commit -- <new-file>                       # fails — pathspec must be TRACKED, so add first
```

New files need **both**, in that order. And export `OMP_MSG_SRC=<msgfile>` when committing with
`-F`, or the commit-msg hook refuses with `COMMIT-MSG REFUSED: round-trip: no message source found`
— a refusal that scrolls past while the agent believes the commit landed. **Read the sha back.**

---
## What counts as a HUMAN DECISION (binding) — and the escalation queue currently contains none

**Measured 2026-09-07, on Joshua's challenge that agents *"push a lot to human decisions, which
aren't actually human decisions at all — it's a cop-out."* He is right, and the mechanism is worse
than a habit: it is AUTOMATED.**

`docs/decisions.jsonl`, 56 rows, resolving the append-log properly (12 rows carry `answers->`
pointers; counting the `decision` field alone overstates the backlog at 35):

```
25 genuinely open
  13  "Bead X cannot be dispatched: its receiver agent is missing. Assign a holder, re-scope it, or park it?"
   8  "Bead X has exhausted its ack retries and is awaiting a human. Assign a holder, re-scope it, or park it?"
   4  substantive
```

**21 of 25 (84%) are auto-generated from two templates, and "assign a holder" is the orchestrator's
entire job.** A missing receiver means the dispatcher found no pane; the remedy is to dispatch it.
None of those 21 is a decision — they are orchestrator work, auto-forwarded to a human at machine
speed. And 26 beads carry the same claim in prose, **10 of them P0**.

**All four substantive rows fail the test too:** `HD-0020` asks whether to lift the build freeze
**while quoting Joshua saying "lift the freeze"** — a recording gap. `HD-0032` re-asks a decision
already recorded as `HD-0008` *"push to public origin/main as-is"* — an execution gap, still
unexecuted at 649 commits. `HD-0030` asks whether to wire a gate with no reachable trigger — the
third rule of this file already answers that. `HD-0019` is a methodology choice an agent can make
and record.

### THE TEST

A decision is HUMAN only if it requires one of:

1. **AUTHORITY** — it reverses or sets a policy the human stated, spends money, or accepts risk on
   his behalf. *"Re-enable a disabled Mac worker"* qualifies; the ruling exists precisely because
   agents kept routing around it.
2. **EXCLUSIVE CAPABILITY** — only he can perform it: a TCC grant, physical access, credentials, a
   purchase, an account he controls.
3. **TASTE ON A PUBLIC ARTIFACT** — what the product *is*, or what ships under his name.

**Everything else is agent work**, explicitly including: ambiguity; two defensible options; *"this
needs a ruling"*; a missing assignee; re-scope-or-park; an UNMEASURED surface — **measure it**; an
unwired gate — **wire it**; a stale document — **update it**.

### THE OPERATIONAL FORM

**An agent may not escalate without first stating the decision it WOULD make and why. If it can
state that, it must make it.** A valid escalation reads *"I cannot act because <clause 1/2/3>"*, and
names the clause. *"This needs Joshua"* is not an escalation; it is an unassigned task with a
person's name on it.

**⛔ AND THE CLAUSE MUST BE UNAVOIDABLE, NOT MERELY PRESENT. Measured 2026-09-07: I failed this
twice in one hour, on the two items I had just written the test to prevent.**

Joshua, verbatim: *"these dont seem like me items — you keep coming back to me items on these — why
exactly are they mine and nt something you all can handle and choose the best statistical option"*.

**A clause licenses an escalation only when EVERY admissible option requires it.** I was treating
*"the goal touches a stated policy"* as clause 1, when clause 1 asks whether **the action I intend**
reverses one. Both items had an in-policy option I had already identified and did not take:

|item|what I escalated|the in-policy option I had already named|
|---|---|---|
|`lppp` half A|*"re-enable a Mac worker — reverses the zero-local-builds ruling"*|**gate the four Darwin tests with a typed `cfg(target_os)` skip.** Pure agent work, no lane, and it makes the verdict HONEST where a Linux failure was masquerading as evidence about Darwin|
|install `finding`|*"the freeze's no-install clause, and a Mach-O needs the local Mac"*|**don't install.** `%20` filed two beads with full acceptance by executing the kernel's SEQUENCE by hand. The install buys *enforcement*, not capability|

**Both are now decided by pane1 as `HD-0045` and `HD-0046`, with the options weighed in the row.**

**Why the in-policy option is not a compromise.** On `lppp` the property is UNVERIFIED either way —
re-enabling a Mac would verify it, gating it would not. But **reporting it as UNMEASURED is TRUE
while reporting it as FAILED is FALSE**, and the false red is what trains operators to ignore a
suite. So the cheap option is also the honest one, and the expensive option buys verification of a
property that has never bitten us.

**The mechanical form: before escalating, enumerate the options and state why each in-policy one is
inadmissible.** If you cannot, you have a decision, not an escalation. An escalation whose row does
not list a rejected in-policy alternative is incomplete — and if the human has to ask *"why is this
mine"*, the answer was in the options you never wrote down.

**NO-CLAIM.** This makes an avoidable escalation *visible in the row*, not impossible — an agent can
still enumerate options dishonestly, or miss one. What it removes is the specific failure where the
option was already identified in the agent's own analysis and simply not chosen.

**Corollary — a decision already given must be EXECUTED, not re-asked.** Both recording gaps above
had the human's own words in the row. Re-asking a settled question is the same failure as escalating
an unsettled one: it converts his answer into another item in his queue.

**Corollary — one label must not cover two halves.** `lppp` is the specimen: *"no Darwin Rust lane
is schedulable"* is genuinely clause 1 (re-enabling a worker reverses a stated ruling), while *"four
tests assert Darwin process-group semantics and run on Linux"* is a test-scoping defect and pure
agent work. Labelling the whole bead `JOSHUA-DECISION` **at P2** stalled both halves at a priority
no selector offers. **Split the bead; escalate only the half that needs a clause.**

**NO-CLAIM.** This test makes a cop-out *nameable*, not impossible — an agent can still assert
clause 1 falsely, and nothing here detects that. The auto-generated templates are the tractable half:
a dispatcher that cannot place a bead should re-queue it or file it as orchestrator work, and it
should be unable to emit a row whose only options are the orchestrator's own verbs.


## Honest limits

- ⛔ **CORRECTED 2026-09-07. THE RETIRED CLAIM WAS "Nothing here is installed. The binary does not
  exist." IT IS FALSE AND IT BLOCKED A CONTRACT LEG.** `%7` reported `41li`'s installed-root-trigger
  leg as unprovable *"because AGENTS.md says this repo has no installed binary"* — reading the file
  correctly and being defeated by it. Measured:

  ```
  command -v ompo   /Users/josh/.local/bin/ompo
  file              Mach-O 64-bit executable arm64, 896,128 B, built 2026-09-07 09:01
  ompo capabilities --json   exit 0
  ```

  **`ompo` is installed, cross-built on Contabo with zero local builds.** This is the THIRD stale
  doctrine row hit in one session — after `tjxt`'s *"`crates/ompo-start` does not exist"* (it landed
  the next day, blocking 43 rows) and `refill-idle-panes`' *"carries only control-plane paths"* (fixed
  Sep 1, obeyed for a day after). **A row asserting a thing is missing licenses routing around it
  indefinitely, so it MUST be re-measured before it is obeyed** — and this file's own warning applied
  to this file, for the third time.
- **AND THE INSTALLED ARTIFACT IS A STALENESS ORACLE, NOT A SOURCE ORACLE.** Measured the same hour:

  ```
  cargo metadata bin targets   88
  installed binary (09:01)     86     <- predates two crates landed since
  fresh build (cargo run)      88
  ```

  **That fully resolves `1io2`'s "hand-typed registry drifted BOTH ways".** The roster is
  `build.rs`-GENERATED (`umbrella.rs:20`, `include!(concat!(env!("OUT_DIR"), "/adapters.rs"))`) with
  anti-vacuity already present at `:191`; the generator was never wrong. **A compile-time-generated
  const is a transcribed value with extra steps** — it reports the workspace as it was when the
  binary was built, so every crate added after an install is invisible until reinstall. **A parity
  leg MUST read a fresh build; a figure from the installed binary is labelled INSTALLED or it is
  wrong.**
- The crate table is now source-audited against control-plane `src/lib.rs`/`src/main.rs` and
  re-runnable inventory commands. The audit establishes description alignment only; it does not
  establish runtime correctness, wiring, or future source drift.
- The no-shell gate covers **file extensions**. It does not prove no crate shells out at runtime via
  `std::process::Command` — a separate, unbuilt check.
- The unwired-lane conformance test described above is **doctrine here, not yet code**. Until it
  exists, "wired" is checked by hand.

## Grading gate: no bead closes on its own author's word

> ✅ **INDEPENDENCE RULE RELAXED — Joshua, 2026-09-07:** *"we dont have to cross lineage, all work
> seems to be going to opus right now — any work can grade or produce as long as its not same
> pane."*
>
> **The bar is DIFFERENT PANE, not different model lineage.** `%19` may grade `%20`; `%7` may grade
> `%8`; any pane may grade any other pane's work. **A pane still may not grade its own.**
>
> **Supersedes** the 2026-09-06 ruling that review independence required a different model lineage.
> That rule was correct for a 4-lineage fleet and became the binding constraint once the fleet
> converged on two — with 3 Claude and 2 Codex, Codex output was gated on Claude availability and
> two same-lineage panes could not clear each other's queues. Control-plane measured the same wall
> at 2+2 and called it *"the NORMAL CASE, not an edge case"*, with a pane idled 47 minutes on it.
>
> **What does NOT change:** a bead is still closed by an agent who did not implement it; the grader
> still **re-runs** rather than reads; the close reason still starts `MUTATION-VERIFIED` / `DONE` /
> `APPROVED` / `WONTFIX`; and the status is still read back, because a prose reason is refused by
> policy and the refusal scrolls past in-pane.
>
> **And `review-lineage-check`'s `LINEAGES` const still keeps all three** — Grok included. This
> relaxes who may be ASSIGNED a grade; it does not retroactively invalidate a recorded review.
> Availability and validity remain different facts, and deleting a lineage to tidy a list would
> destroy correct history — including the Grok review that caught the malformed ACK token in this
> file's own dispatch doctrine.

**⛔ ONE CLAUSE INSIDE THAT QUOTE IS FACTUALLY WRONG, AND THE POINTER LANDS HERE RATHER THAN
DOWNSTREAM.** Line 2006 says *"a prose reason is refused by policy and the refusal scrolls past
in-pane."* **Nothing refuses it.** `crates/ack-spine/src/close_reason.rs:106-109` documents its own
bypass, `CloseReason` references outside that crate measure ZERO against a positive control of 14,
and **6 of 241 closed beads carry a non-conforming prefix — one of them EMPTY.** Corrected in full
under *Grading gate* below; tracked as `uqnut`.

**The quote is left byte-intact on purpose.** It is a verbatim record of a ruling, and editing a
quoted ruling to repair a factual error inside it destroys the record. **The pointer sits at the
original text because this file has already paid for the alternative:** `docs/plan/flow/CONTRACT.md`
said **BUILD FREEZE** in bold at `:59` while the amendment lifting it for S1 sat at `:101`, and a
reader who stops at the word FREEZE never reaches the word AUTHORIZED. **That cost a session:** 131
authorized beads sat claimable while three panes were routed to audits.

**A bead is closed by an agent who did NOT implement it.** Verification runs
`/beads-compliance-and-completion-verification` against the bead's own acceptance
criteria, and the close reason cites what the GRADER re-executed — not what the
implementer reported.

Measured 2026-08-31: of the first three closes, one was independently graded (`-4ak`,
pane 1, "re-run, not read") and two were self-certified by their implementer (`-7ai`
GoldLark, `-a3p` BlueLantern). Both self-closes carried real evidence and a NO-CLAIM
line, which is why this is a process gap and not a fabrication — but a report is a
CLAIM, and the whole point of the grade is that a second agent ran the command.

**Stage order, and grading outruns new work.** When a bead is implementation-complete,
grading it takes priority over claiming anything new. A verification backlog is worse
than an empty queue: unclosed finished work makes `br ready` keep serving it, which is
how a pane ends up correctly reporting NO_ELIGIBLE_TARGET and going idle.

Every bead carries a stage, and dispatches name it:

```
IMPL     implementation-complete, commit landed
  ->  GRADING   a DIFFERENT agent re-executes the acceptance criteria
  ->  CLOSED    the grader closes; reason starts MUTATION-VERIFIED / DONE / APPROVED / WONTFIX
```

The IMPL→GRADING edge is owned by `crates/dispatch-saga` (`dispatch-saga grading-transition`).
Decide-only by default; `--apply` may `br update --status grading`. A pane MUST NOT move its
own bead (that is self-certifying the stage). The grader still closes. Caller: `ack-stage`
(`impl_to_grading_after_ack`).


**CORRECTED 2026-09-08 — THIS PARAGRAPH ASSERTED A REFUSAL THAT DOES NOT HAPPEN, and the code was
the honest party the whole time.** The retired claim was *"the close policy REFUSES a prose reason,
and the refusal scrolls past in-pane while the agent moves on believing the close landed."* **There
is nothing to scroll past on a direct `br close`.** `crates/ack-spine/src/close_reason.rs:106-109`
says so in its own doc comment on the `PolicyRefused` variant:

> *"The tracker command itself stores arbitrary reasons, so a direct tracker close can bypass this
> verdict; the status must be read back rather than inferred from the command appearing to succeed."*

**So ack-spine types eight prefixes, carries a CLOSE_REASON_POLICY_REFUSED verdict, and names its own bypass.**
The classifier is consumed by pre-delete-citation-check::check_close_reason_policy, and the
pre-commit gate invokes that reader when .beads/issues.jsonl is staged. Direct br close still
stores arbitrary reasons; the gate is DETECTION, not PREVENTION, and catches the bad row on the
next mirror-staging commit rather than preventing the original close.

**The historical measurement below predates this caller:** 6 of 241 closed beads carried a
non-conforming prefix, one EMPTY, and nothing refused them. Those existing closes are not
retroactively reopened; future staged mirror reads report the denominator and refuse new drift.
**WHAT IS ACTUALLY TRUE, and it is the only durable half: ALWAYS READ THE STATUS BACK.**

```
br show <id> --json | jq -r '.[0].status'
```

`br show` returns a BARE list while `br list` wraps its rows in `.issues` — **and `br list` EXCLUDES
closed rows by default**, which made a grading-backlog query report `closed today: 0` against a real
figure of 17. Tracked as `uqnut`, whose measured decision is to EXTEND the prefix set rather than
force five more-precise verdicts (`PREMISE-FALSE`, `MUTATION-NOT-REQUIRED`, `MUTATION-ATTRIBUTED`)
behind `WONTFIX`.

---

## Graph and evidence rules (receipts: [LIFECYCLE-FAILURES.md](docs/negative-patterns/LIFECYCLE-FAILURES.md#three-graph-and-evidence-rules-that-strangled-real-work-tonight))

**Keep ownership in `parent-child` edges and never put a `blocks` edge from an epic onto its own
leaf** — that is circular by construction and it strangled 13 of the first 30 open beads, four of
them P0. **The authority is ATTEMPTING the transition and reading the refusal**, not `br show`.

**Audit CLOSED beads for a path before you `git rm` it.** A closed bead's cited evidence is a live
filesystem dependency, not a historical note.

**Read a consumer's extractor and its scope before scanning for it.** Measured: a scan sized from an
inferred regex overstated the problem ~13×, and 47 of 71 "unresolvable" citations were a regex
artifact rather than broken evidence.

## Post-mortem: the fleet idled 6+ hours while every watchdog fired (receipt: [LIFECYCLE-FAILURES.md](docs/negative-patterns/LIFECYCLE-FAILURES.md#post-mortem-the-fleet-went-idle-for-6-hours-while-every-watchdog-fired-2026-08-31-session-post-wave))

**When admission is red, route typed low-stakes work — grading, verification, hygiene — instead of
naming the blocker forever.** Detection fired correctly for six hours; the response layer did not
exist, because every lane fail-closed on admission and nothing was authorized to act on a DEGRADED
signal. **Route by WORK LOCATION, not by session membership**, and treat a staleness metric that
re-fires inside 25 minutes as a defect in the metric.

---

## KERNEL-ONLY (binding): you may not handroll a capability a kernel provides

**We build the system and then do not use it.** Measured 2026-08-31, five handrolls in one
session — by the author of the kernels:

| job | what I did | the kernel that already existed |
|---|---|---|
| observe panes | `tmux capture-pane \| grep -oE` for 12 hours | **`tick-monitor observe`** — installed, and returns *more*: state, timer, liveness, attention, dead panes, correct session scoping |
| dispatch | raw `tmux send-keys` | **`ntm --robot-send`**, `refill-idle-panes`, `fast-dispatch`, `controller-tick`, `loop-driver` — installed. **NOT cron-scheduled: corrected 2026-09-07.** The only crontab line naming any of them is a COMMENT (*"refill-idle-panes refused 51 ticks, 2026-09-02"*) — an epitaph for rows that were removed. Per the corrected census in rule 9, 23 of 36 bin kernels have a `.flywheel/` document mention and **no executor**. A handroll is still worse than the kernel, but the kernel is not firing on its own either |
| receipt | `grep -oE` on a timer | **`receiver-receipt`** |
| file a bead | raw `br create` | **`crates/finding`** — which I wrote *thirty minutes earlier* to make an unfiled gap impossible, then bypassed in the next tool call |
| read the queue | `br ready --json \| python3` | **`bv --robot-triage`** — the planning brain, which reports scores the raw query cannot see |

My hand-grep silently mixed a control-plane pane into an omp-orchestrator census, because it never
scoped. The kernel does. **A handroll is not merely redundant — it is usually worse.**

### THE RULE: if a kernel is broken, FIXING IT IS THE WORK

This is the clause that matters, because it names the mechanism rather than the symptom.
`refill-idle-panes` carried **only control-plane paths** (measured via `strings` on 2026-08-31), so
it supervised the wrong repo all night. Rather than fix one default, I hand-dispatched for hours.

**CORRECTED 2026-09-02 — the kernel was FIXED and the doctrine outlived the defect.** The binary
was rebuilt Sep 1 19:29 and now carries no `/Users/josh/Developer/*` literals at all
(`strings … | grep -oE '/Users/josh/Developer/[a-z-]+'` returns nothing). `--plan` from this repo
correctly resolves `bead=omp-orchestrator-omp-surface-map-41b`, and the `--apply` lane is cron'd at
`8,28,48` and alive — its log reads `no idle pane both surfaces agree on — nothing to do`, which is
the two-surface agreement rule working, not a silent failure.

**The lesson survives the fix, and got sharper.** A stale "this kernel is broken" note is itself a
reason to handroll — it licenses the routing-around indefinitely, long after the kernel is sound.
**A doctrine row asserting a kernel is broken MUST be re-measured before it is obeyed.** Measured
the same session: I handrolled `tick-monitor`'s job with `capture-pane | grep` for hours on the
strength of notes like this one, while `tick-monitor observe` was installed, correct, and returning
strictly more — `state`, `timer_secs`, `liveness`, `attention`, `dead_panes`, `omp_lifecycle`,
`git_commits`, correctly session-scoped. Its first invocation reported `gap_secs=7773`: **nobody
had observed a tick in 2.2 hours.**

One live defect remains, and it is small: `refill-idle-panes --plan` proposes `pane=1`, the
ORCHESTRATOR pane. The orchestrator must be excluded, and no `OMP_*` exclusion variable appears in
the binary's strings. **That is the fix to make — not a reason to hand-dispatch.**

> **Every handroll is locally cheaper and removes exactly the pressure that would have fixed the
> kernel.** That is why the kernels stay broken. Routing around a broken kernel is not pragmatism;
> it is the thing that guarantees the next agent finds it broken too.

The same shape, three ways in one session: prose instead of beads; `br create` instead of `Finding`
(the standard exists as a type, unenforced); hand-grep instead of `tick-monitor` (the census exists,
unqueried).

#### ⛔ CORRECTED 2026-09-07 — "`crates/finding` exists, ZERO CALLERS" IS FALSE IN BOTH HALVES

**Retracted:** *"`crates/finding` exists, zero callers"* and the row above reading *"which I wrote
thirty minutes earlier to make an unfiled gap impossible, then bypassed in the next tool call."*
Caught by `%20` and re-measured:

```
external manifest callers                    14
src references (`finding::`)                 17   across 15 crates
  ack-spine, crate-atom-gate, dispatch-silence-watch, fast-dispatch, finding-dispatch,
  fleet-truth, loop-coverage, loop-tick, no-shell-gate, omp-idle-dispatch,
  omp-inventory-map, omp-orchestrator, pre-delete-citation-check, refill-idle-panes,
  verify-dispatch

[[bin]] in Cargo.toml                         0
src/bin/ entries                              0
src/main.rs                                   ABSENT
`finding` on PATH                             ABSENT
```

**It is WELL WIRED as a library and has NO OPERATOR SURFACE.** An agent at a shell **cannot invoke
it**, so `br create` is the only path available — and that **inverts this rule's premise.** The
failure was never *"the author bypassed his own kernel."* It is **"the kernel has no operator surface
to bypass."**

**The verdict class was wrong, per gate rule 4a (`fh C69`).** "Zero callers" is an **ABSENT** verdict
— *build the callers*. The truth is a **MISSING BIN** — *build the operator surface*. Fourteen
manifest edges were sitting there the whole time the row said none existed.

**And this is the SECOND stale broken-kernel row hit in one session**, after `refill-idle-panes`,
which this file already records as *"CORRECTED — the kernel was FIXED and the doctrine outlived the
defect."* This file's own warning applies to itself: **a doctrine row asserting a kernel is broken
licenses routing around it indefinitely, so it MUST be re-measured before it is obeyed.** Two rows,
one night, both stale in the direction that excuses a handroll.

⛔⛔ **RE-CORRECTED 2026-09-11 — THE "LIVE GAP" CLOSED FOUR DAYS AGO AND THIS ROW OUTLIVED IT.
THIS IS THE THIRD STALE BROKEN-KERNEL ROW, AND IT IS IN THE PARAGRAPH WARNING ABOUT THE SECOND.**
The retired text read *"the live gap is a missing `[[bin]]` on `crates/finding`."* Measured:

```
grep -c '[[bin]]' crates/finding/Cargo.toml        1     <- name = "finding", path = "src/main.rs"
command -v finding                    /Users/josh/.local/bin/finding
finding --help                        "usage: finding <subcommand>"   rc=0
landed                                c79524e  2026-09-07  "give the finding kernel an operator surface [test]"
```

**The operator surface EXISTS, is installed, and runs.** So the corrected row's own remedy was
executed the next day and the row kept telling readers to build it — the identical failure it was
written to name, one level down. **A correction is a value and goes stale exactly like the claim
it replaced.** Bead `ca9q` ("the finding crate is unrouted: `file()` had zero callers") is
therefore `ALREADY-FIXED` on both halves: 14 manifest callers, 16 src refs, and a live bin.

 **The re-measurement cost one command. Obeying the row would have cost a build.**

### The full loop, demonstrated end to end through kernels only

```
observe   tick-monitor observe --session omp-orchestrator   → dispatchable/free/attention/dead
dispatch  ntm --robot-send=omp-orchestrator --panes=5       → {"success": true}
receipt   tick-monitor observe                              → %1409 WORKING t=6
```

`t=6` from a previously IDLE pane is an **idle→working transition with a fresh timer** — the
strongest receipt available, per the receiver-receipt contract. No `capture-pane`, no `grep`, no
`send-keys`.

### Enforcement, because a written rule has failed five times tonight

- **Source half** — a gate scanning tracked files for handrolled equivalents, emitting `file:line`
  **and naming the kernel that should have been used** (a finding that does not name the
  replacement is not actionable). Its known-good leg is mandatory and non-obvious: the **kernel
  crates themselves must pass**, since `tick-monitor` legitimately calls tmux and
  `subprocess-contract` legitimately spawns — via a **declared, not inferred** allowlist.
- **Operator half** — the gate can only see committed source. It **cannot** see an operator
  handrolling in a shell, which is how all five above happened. That needs a `PreToolUse` hook and
  is a separate bead. **The source gate must say so in its own output** rather than implying
  coverage it does not have.

## Hook coverage window (binding)

A PreToolUse hook table is read when an agent session starts. Installing or changing a hook does
not retrofit already-running agents. A session that predates the installation is UNCOVERED until
it restarts; zero denials is not evidence that the hook ran.

Hook evidence has two separate levels. Piping a JSON event into a hook binary proves only the
binary decision. Hook-level coverage requires a real tool call from the agent session that the
hook intercepts and denies. A report may claim interception only when that real call was refused;
binary probes must be labelled binary-only.

The local kernel-only hook records a per-session invocation count in its shadow ledger. Run
kernel-only-operator-hook --liveness SESSION_ID BASH_CALLS after the configured threshold
KERNEL_ONLY_HOOK_LIVENESS_THRESHOLD (default 10). It emits HOOK_LIVENESS UNCOVERED and exits 1
when Bash calls reach the threshold with zero recorded hook invocations. Shadow ledger evidence is
diagnostic, not hook certification.

CORRECTED 2026-09-05 -- THIS PARAGRAPH ASSERTED AN OUTCOME THE CLASSIFIER COULD NOT PRODUCE, which
is the more dangerous half of a doc-vs-code drift: a reader checking whether this hole was closed
found a sentence saying it was. The retired claim was "COVERED, PARTIAL, and UNKNOWN remain
distinct outcomes." Measured at crates/kernel-only-operator-hook/src/shadow.rs:233-239, the state
machine had exactly THREE values and COVERED was the fallthrough:

    let state = if bash_calls >= threshold && hook_invocations == 0 { "UNCOVERED" }
                else if hook_invocations < bash_calls { "PARTIAL" }
                else { "COVERED" };

With bash_calls = 0 and hook_invocations = 0 against any nonzero threshold, the first guard is
false and 0 < 0 is false, so a session that NEVER RAN classified as COVERED. UNKNOWN appeared once
in the whole crate, at src/main.rs:226, and only for a probe ERROR -- so "the probe failed" and
"the probe ran and found no evidence" had one representation between them, which is this file's own
denied-probe rule violated inside our own kernel. Found by pane %9 on omp-orchestrator-j2z9 and
verified independently at source by pane 1.

The fix was to EXTEND the kernel rather than write a second crate, per the KERNEL-ONLY rule that
fixing a broken kernel IS the work: classify_hook_liveness now has three POSITIVE-EVIDENCE arms
(UNCOVERED / PARTIAL / COVERED) and every residual -- including zero-and-zero -- is UNKNOWN.

The general lesson outlives this function. A CLASSIFIER WHOSE COMPLIANT VERDICT IS THE DEFAULT
BRANCH RE-ACQUIRES THIS DEFECT EVERY TIME SOMEONE ADDS A CASE. Each arm must assert its own
precondition and the residual must be the unknown, never the pass. Fail closed in the direction
that costs nothing.

The installed rch-lane-bind binary at /Users/josh/.local/bin/rch-lane-bind is outside this repo.
Its stdin probe cannot prove that the Claude PreToolUse table intercepted a live call, and this
repo does not claim fresh-session coverage from a stale session. The measured wrapper limitation
is named rather than hidden: timeout, nice, env, time, stdbuf, and xargs wrappers may bypass a
resolver that inspects only the command-position token. Until a real intercepted probe closes
that class, wrapped invocations remain UNMEASURED/UNSAFE for coverage claims.

---

## An arm64 artifact that is MANDATORY and UNREBUILDABLE (binding, measured 2026-09-11)

**Joshua forbade darwin builds. Two artifacts on this machine are Mach-O arm64, are required for
normal work, and can only be refreshed by a darwin build. They are now frozen at whatever version
they happened to be when the ruling landed.** This is not a complaint about the ruling; it is the
consequence the ruling has, stated once so nobody rediscovers it at 23:00 with the fleet stopped.

```
.git/hooks/pre-commit          Mach-O arm64   REFUSES EVERY COMMIT when it judges itself stale
/Users/josh/.local/bin/ompo    Mach-O arm64   the operator verb every wiring proof terminates at
```

**Both surfaced within one hour, independently, from opposite directions** — the hook froze the
whole checkout, and `WireGradingPacket` could prove a call chain reaches `ompo supervise grade`
in-tree and on Linux while being unable to put the call site *into the installed binary*. **Two
surfaces, one cause.** A wiring proof that terminates at an installed binary now has a residual it
cannot close: **the chain is REACHABLE and the shipped artifact PREDATES it.** Say so in the
NO-CLAIM rather than calling the path live end-to-end.

⛔ **THE DOCUMENTED REMEDY IS A BRICK TRAP. DO NOT FOLLOW IT.** `hook_freshness.rs:163`/`:203`
says `cargo build --release --bin pre-commit-gate && cp target/release/pre-commit-gate
.git/hooks/pre-commit`. **Under CONTABO-OR-BUST that build lands on a Linux x86_64 worker, and
copying a Linux ELF onto the hook does not produce a stale hook — it produces an UNEXECUTABLE
one, and then every commit in the repo fails to exec.** Found by `TautologicalGuard`, which
stopped at the trap instead of running it. **The repo has already paid for this class once:** an
x86-64 ELF reached this arm64 host and five ledger writes died with `cannot execute binary file`.
**The remedy line silently assumes a LOCAL build, so it is unavailable to an agent under the build
policy — and following our own documentation would have converted a refusal into a hard brick.**

### `hook_freshness` itself: four defects, all measured the night it stopped the fleet

1. **IT READS THE WORKTREE AND REFUSES COMMITS.** `commit_ratchets.rs:154-193` compares the hook's
   mtime against the newest `.rs` under five `HOOK_SOURCE_CRATES`. **Any agent's UNCOMMITTED save
   refuses EVERY other agent's commits, including commits touching none of those files.** The two
   blocked commits were `AGENTS.md` and `loop-queue-filter`. **This is rule 8 — `cargo` reads the
   worktree, a sha names a tree — living inside a gate.** Measured: the hook was **36 hours NEWER
   than its committed source**; only uncommitted WIP was newer.
2. **`src/` ONLY, NEVER `tests/`.** `newest_hook_source:187-194` walks `crates/<c>/src`.
   **Positive control:** `no-shell-gate/tests/build_identity.rs` (22:42:49) was newer than the
   20:29:17 hook **during a CLEAN observation.** Derived independently by three agents. An owner
   who only touched tests will wrongly self-identify.
3. ⭐ **IT IS A WHACK-A-MOLE ORACLE: it reports only `newest_path`, the MAX.** It named
   `gate-reachability.rs`, then `project_agent.rs`, then `commit_ratchets.rs`. **Three agents were
   each handed a different filename; landing any one unblocks nothing because the next-newest takes
   its place, and nothing tells you the gate has moved on.** Self-concealing, not merely slow.
4. ⭐ **THE CLEARING OPERATION IS SLOWER THAN THE CONDITION THAT TRIPS IT.** `TautologicalGuard`'s
   framing, and it is stronger than "self-sealing gate": a clear at 23:18:48 **re-armed 13 seconds
   later**. No number of reinstalls converges against a live editor. **The remedy is a QUIESCE
   REQUEST to the owner — a fleet-coordination act, not a technical one.**

⛔ **AND IT REFUSES THE OWNER'S OWN FIX.** muse could not commit the very edits that armed the
gate, because the refusal reads worktree mtimes and its files were the worktree. **The one agent
positioned to end the freeze by landing was the one agent the gate blocked from landing.** I
offered "land it" as an option; muse measured that the option could not exist.

**WHAT AN AGENT MAY DO WHEN THIS FIRES — nothing else:**

```
NEVER  touch the hook              mtime is not provenance; it hides a genuinely stale gate
NEVER  git commit --no-verify      skips EVERY gate, not the one that fired
NEVER  rebuild + reinstall         darwin build (forbidden) or an ELF brick (worse)
DO     find the owner and ask them to quiesce or land, then tell pane 1
```

**FIND THE OWNER FROM THE DIFF, NOT THE TRACKER.** muse was identified in minutes because it had
written the bead id into the source it was editing:
`git diff -- crates/no-shell-gate | grep -c '^+.*9ub39'` → **5**, negative control `ZZQ-NOT-REAL`
→ **0**. **Anchor to `^+` — an unanchored `grep -c` counts context lines** and two agents got
different sizes for the same diff that way.

**Clearing the gate is the CONDUCTOR's act and carries a mandatory disclosure:** the sha256 before
and after (**must be identical** — mtime only), the hook's mtime against its **committed** source,
and the statement that every other gate stayed live. **It is defensible only while the hook is
newer than committed source.** If it ever genuinely predates committed source, there is no
in-policy clear and the freeze is real.

**NO-CLAIM.** This documents the collision; it does not resolve it. The durable fix — scope
`hook_freshness` to staged paths, or to the hook's dependency closure, or emit the full
over-threshold set — **cannot take effect without rebuilding the hook**, which is the forbidden
operation. **A gate whose only escape hatch is a forbidden operation will stop the fleet every time
it fires**, and it has now done so once.

### A tracker assignee is not a hub id — the phantom-claim probe is RETRACTED

**I ruled that an assignee absent from `hub list` is a phantom holder, and retired claims on it.
It is wrong, and it was wrong in the direction that destroys live work.** **TRACKER ASSIGNEE
STRINGS AND HUB IDS HAVE NEVER BEEN THE SAME NAMESPACE.** Measured **n=3, across three unrelated
beads, by three agents who did not share a subject:**

```
bg-grade-uldvu-real   ->  GradeUldvuP0        live, working the bead
bg-grade-poumg7       ->  GradePoumgFamily    live, working the bead
bg-uldvu-3            ->  UldvuSchedulerProbe live, awaiting a non-author grade
```

**The probe cannot return the other answer** for a correctly-claimed bead whose claimant did not
happen to name itself after its hub id — the instrument-defect shape (`8i`). **The corrected
oracle: NO LIVE AGENT ANSWERS FOR THIS BEAD.** Being right once by coincidence — one displaced
claim really was dead — does not validate it; that was established by OUTCOME (no commit and no
comment after the grade), never by hub-absence.

⭐ **STANDARD ADOPTED: WRITE THE HUB ID *AND THE COMMIT* INTO THE BEAD.**
`UldvuSchedulerProbe`'s rule — the first comment on any claim binds
`tracker assignee ↔ hub id ↔ commit ↔ state`. It generalises the thing that found muse in minutes
(*the bead id was in the diff*) into *the hub id is in the bead*. **Verified discoverable, not
merely local:** `grep -c UldvuSchedulerProbe .beads/issues.jsonl` → 1 and `br show` renders it, so
it reaches a grader on another path.

⛔ **AND THE AUTHOR OF THE STANDARD NAMED ITS FAILURE MODE BEFORE IT HARDENED — IT IS THE
RETRACTED PROBE WITH THE SIGN FLIPPED.** A binding comment is **static text written once at claim
time and never revoked**, so if its author dies, is reaped, or parks, the comment still reads
*live, awaiting grade*. **The hub probe could only ever answer PHANTOM; the binding comment can
only ever answer PRESENT.** Both are single-valued oracles. It is still a strict improvement —
**under-retiring is the cheaper error**, since a stale bead merely waits while a wrongly-retired
one loses committed work, which nearly happened three times in one night — but what it buys is
narrow: **it resolves the NAMESPACE gap, never LIVENESS.**

**So the oracle has three legs, and the third is not a liveness question at all:**

```
namespace   the bead names its hub id            binding comment, static, now standard
liveness    that hub id is in a FRESH `hub list` re-measure AT GRADE TIME, never cached
survival    the named commit exists in git log   <- THE ONE THAT PROTECTS THE WORK
```

⭐ **IF THE COMMIT EXISTS, GRADE IT — NEVER RETIRE IT.** The deliverable outlived its author and
the bead needs a grader, not a sweep. A bead is genuinely abandoned only when its named hub id is
absent from a fresh roster **AND** its stated commit is not in `git log`. Absent the third leg,
every claim-retirement sweep is one instrument away from deleting landed work.

⛔ **BUT THE COMMIT PROVES SURVIVAL, NEVER DISPOSITION — AND `git log` ALONE DECIDES WRONG HALF
THE TIME.** `UldvuSchedulerProbe` audited its own row against the leg it had just contributed and
found the mirror image of the case below:

```
a4468ad   release beat in the COMMIT MESSAGE, absent from the bead   -> tracker-only reader decides wrong
948236e   release beat in the BEAD comment, absent from the message  -> git-log-only reader decides wrong
```

**Two observed cases, and the release beat sits in EXACTLY ONE of the two surfaces each time —
a DIFFERENT surface each time.** That is a shape, not two anecdotes. **Read leg 3 ALONGSIDE the
bead, never as a substitute for it:** the commit answers *did the work survive*, the bead answers
*was it released*, and neither answers the other. A reader taking *"the named commit exists"* as
licence to decide from `git log` alone gets `948236e` backwards — a landed commit with no stated
disposition reads as work-in-flight when it is in fact awaiting a non-author grade.

⛔ **AND SURVIVAL DOES NOT RE-PROVE GREEN.** `VerifyDispatchGap`, declining to assert it: the diff
landed, is pushed, and is byte-unchanged `git diff --numstat a4468ad HEAD -- <path>` → EMPTY —
**but 12 commits have landed since its last run, so "still green" from it now would be a
transcribed value**, which this file already forbids. **Survival is about the diff; green is about
the tree, and a grader that wants green re-runs it.**

**AND AN AUTHOR'S RELEASE IS OFTEN ONLY IN THE COMMIT MESSAGE.** `a4468ad` ends *"Bead:
omp-orchestrator-poumg.7 (left in_progress for a non-author grade)"*. That is the **RELEASE beat**
of `file → claim → dispatch → ACK → observe → verify → RELEASE → close`, written into the commit
**because the tracker has no field to record a release in** — which is exactly why such a bead
reads as unheld. Two panes refused grades the same week for the inverse (an author still holding
`assignee`), so the gap cuts both ways.

⭐ **THE SHARPEST STATEMENT OF WHY ALL THREE LEGS EXIST came from a subagent reasoning about its
own mortality.** `VerifyDispatchGap`: *"I will not outlive this task, so by the time anyone reads
my binding it is likely false in the only direction that matters — do not cite my liveness line,
re-measure."* **An agent that writes the expiry of its own evidence into the evidence is doing the
thing this whole section is about.** `poumg.7` is the concrete near-miss: under a namespace-only
or liveness-only sweep, **a live grader AND a landed, pushed, unmodified fix would both have been
swept.** **Grade the commit, not the claimant.**

---

### A CITATION MUST NAME ITS SCOPE — one rule at three granularities (binding, 2026-09-11)

**`GradeCatch22`'s unification, and it is why this is one row instead of three.** This file
already says a `cargo` figure must cite a TREE (rule 8). The same discipline, one level down,
says a **commit sha must cite a FILE SET**, and one level down again says a **dependency claim
must cite a MANIFEST BLOB.** They are the same rule: *evidence is scoped to what you actually
measured, and an inference that widens the scope is not evidence.*

```
granularity   the claim                          the ONE command that settles it
tree          "the tests pass"                   which tree? cargo reads the WORKTREE
file set      "commit X landed the change"       git show --numstat X
manifest      "my deps are in the tree"          git show HEAD:<crate>/Cargo.toml
```

⛔ **THE CONDUCTOR BROKE THE MIDDLE ROW THE SAME NIGHT IT WAS WRITTEN.** I read
*"`TautologicalGuard`'s commit landed"* and published a GO on *"the `resident.rs` refactor
landed."* `746a535` touched **`loop-queue-filter/src/{main,select}.rs` and nothing else**;
`render_grading_packet` was **0 in HEAD's `resident.rs` and 4 in the worktree's.** ⭐ **A COMMIT
SHA IS NOT EVIDENCE ABOUT A FILE IT DID NOT TOUCH.**

⭐ **AND IT CUTS IDENTICALLY IN THE REASSURING DIRECTION** — `GradePoumgFamily`, who had HEAD move
under it mid-grade and did the opposite of what I did: *"it is not evidence that a file is
unchanged either. Only the blob answers, and hashing it is one command."* It ran
`git show --stat` and re-hashed three graded blobs; all clean, so its verdicts stood **without
re-running the suite**. Same instrument, opposite inference, **symmetric cost**.

⛔ **THE MANIFEST ROW IS `UldvuSchedulerProbe`'s AND IT IS THE SUBTLEST OF THE THREE.**
*"I added no dep"* ≠ *"my deps are in the tree."* Those are different claims, and only the second
protects you:

```
"zero new `use` lines, no manifest touched"   proves you ADDED no dependency
git show HEAD:<crate>/Cargo.toml              proves the ones you USE are COMMITTED
```

**A crate `path`-added by an UNCOMMITTED PEER MANIFEST satisfies the first argument perfectly and
still yields a borrowed green** — which is exactly the `q1wmb` shape, six uncommitted manifests
desyncing `Cargo.lock` by 59 lines. It caught this in its own landed commit by re-running rather
than re-reasoning, and reported that **the reason it re-ran is the point, not the result.**

⭐ **AND THE ASYMMETRY THAT EXPLAINS WHY A GRADER KEEPS BEING SAFE WHERE A COMMITTER IS NOT**
(`GradeCatch22`, about `WireGradingPacket`'s finding, which it says it would not have caught):
**a grader verifies what a peer ALREADY DID, so a clean worktree plus a green suite genuinely
suffices. A committer's soundness depends on a peer's NEXT ACTION, which no measurement of the
past can answer.** In a zero-worktree single checkout that is not a nicety: `WireGradingPacket`
measured its file clean and the suite at `227 passed / 5 failed` (its known baseline), **asked
anyway, and was told HOLD — the peer's anti-vacuity mutation had not been planted yet.**
Committing on that green was a coin-flip on taking a planted mutant into HEAD. **Three times in
one night, asking beat inferring from a clean measurement.**

**NO-CLAIM.** These are one-command checks that make a scope error *visible*; none of them makes
it impossible, and nothing in-tree enforces any of the three today. The manifest row is a
candidate acceptance leg for `q1wmb`.

---

## A KNOWN-BAD PLANT IS A LOADED GUN IN A SINGLE CHECKOUT (binding, measured 2026-09-11)

**Measured during an open commit window: a `2/2` swap of `record_grader_assignment` against
`render_peer_grade_packet` sat live in `crates/omp-orchestrator/src/resident.rs`, marked
`// M4: ORDER SWAPPED`, unattributed — and TWO agents were wrongly accused of it inside one
hour.** It was almost certainly a grader legitimately re-running an acceptance; the work was
right and the method was unsafe. **Nobody was identified and nobody should be** — the reporter
named the mechanism and explicitly declined to name a culprit after watching two misattributions.

### Rule 1 — announce before you plant, announce after you restore

**A plant must be announced to the fleet BEFORE it is written and its restore announced after,
or it must not be written while a commit window is open.**

⭐ **THE SUBTLE PART, AND IT IS WHY "restore before you hand the file over" IS NOT THE RULE: THE
DANGER WINDOW IS NOT THE HANDOFF. IT IS EVERY INSTANT ANY PEER MAY PATH-SCOPE A COMMIT.** In a
zero-worktree single checkout, `git commit -- <path>` takes **worktree** content, so your mutant
ships **under someone else's name, in a commit they believed was theirs.** The same agent refused
three commit windows that night on exactly this reasoning and was right each time; this is the
identical hazard with the roles reversed.

### Rule 2 — agent-scoped markers, unconditionally: `PLANT-<bead>-<agent>`

**A bare ordinal carries no provenance, and `M4` collided n=2 in one hour on one file.**
`TautologicalGuard` wrote `MUTANT M4`; `GradeCatch22` wrote `M4: ORDER SWAPPED`. A peer read a
live diff, saw `M4`, and accused `TautologicalGuard`; then two agents saw `M4` and accused
`WireGradingPacket`. ⭐ **IN A SINGLE CHECKOUT THE MARKER IS THE PROVENANCE**, and four characters
of shared namespace cost two false accusations. Proposed independently by the two agents burned.

⭐ **THE CAUSE IS MUNDANE AND THAT IS WHY IT WILL RECUR: `GradeCatch22` NUMBERED M1–M5
SEQUENTIALLY ACROSS ITS OWN NIGHT'S MUTATIONS. "MY COUNTER WAS PRIVATE AND THE FILE WAS
SHARED."** Nobody chose a colliding name; two agents independently reached for the obvious one.
**A private sequence in a shared namespace collides by construction**, which is why the marker
must carry the agent, not merely be distinctive.

⛔ **AND A PLANT WITH *NO* MARKER IS STRICTLY WORSE THAN A COLLIDING ONE** — `GradePoumgFamily`,
disclosing its own: a `sed` that deletes a token leaves nothing behind, so **it cannot even be
misattributed; it is simply invisible.** If a peer path-scopes a commit during that window, the
mutant lands under their name with nothing to identify it. **An unmarked plant is undetectable
by every instrument in this file.**

### Rule 4 — when two files can prove the same property, plant in the one nobody is holding

**`GradePoumgFamily`'s practice, and it is the cheapest risk reduction available.** It needed a
known-bad for an arity contract, checked `git status` on `resident.rs` first *because the fleet
had been told `TautologicalGuard` was mid-mutation there*, and **moved its plant to the CALLEE
instead.** The arity leg reddens from either side of the contract, **so the uncontested side was
an equally valid mutation site at strictly lower blast radius.** Ask which files can prove the
property before asking how to plant in the first one you thought of.

### Rule 3 — a suite verdict is NEVER a restore oracle

**`sha256` + `cmp`, always.** On `crates/omp-orchestrator` **a GREEN IS `exit=101`** —
`227 passed / 5 environment-blocked` (`TMUX_PANE_missing`×4 + `br init:
Process(NotFound("br"))`) is the healthy state, so *"the suite went green"* **returns the same
answer whether or not the restore worked.** Same single-valued-probe class as the retracted
phantom check. Three agents reached this independently. The underlying baseline defect is `g5j5b`.

**NO-CLAIM.** These are conventions with no gate behind them: nothing in-tree refuses an
unannounced plant or a bare marker, and the first two depend on every agent reading this — the
enforcement class this repo distrusts. What they remove is the specific ambiguity that made a
live mutant unattributable.

---

## THE CI GATE CANNOT PRODUCE A VERDICT WHILE THE FLEET WORKS (fixed 2026-09-11)

**`REACHABLE_RED_UNREAD` was understated. The verdict was not unread — it was NEVER PRODUCED.**

```
last 15 runs        14 cancelled, 1 running, ZERO verdicts
last REAL verdict   34549975939  failure  2026-09-11T01:16   FOUR HOURS EARLIER
runs since          cancelled at 224-1117s, every one
last 100 runs       51 failure · 48 cancelled · 0 success
```

**Cause: `gate.yml` set `cancel-in-progress: true` on a group keyed by `workflow+ref`, so every
push to `main` killed the running gate.** Five pushes in an hour against a ~20-minute gate means
it never finishes. ⭐ **AND THE RUN EVERYONE CITES AS AUTHORITATIVE IS SIMPLY THE LAST ONE THAT
SURVIVED, NOT THE NEWEST** — the doctrine's oracle (*"newest run whose conclusion is
`success|failure`"*) is a **workaround for this defect**, which is why it was needed at all.

**The old justification was sound and load-bearing on an assumption that failed:** *every job
scans the whole tree, so a later commit's verdict strictly covers the earlier one* — **true only
if the later run FINISHES.**

**FIXED by splitting the two cases**, because they are genuinely different: a superseded **PR**
commit's verdict really is worthless, so PRs keep supersession; a pushed **`main`** commit is
permanent and must keep its own verdict, so its group carries the sha and nothing can cancel it.

```yaml
group: gate-${{ github.workflow }}-${{ github.ref }}-${{ github.event_name == 'push' && github.sha || 'shared' }}
cancel-in-progress: ${{ github.event_name != 'push' }}
```

**Validated with a STRICT duplicate-key loader, per gate rule 6** — `yaml.safe_load` silently
ACCEPTS duplicate keys, so it cannot prove the file parses: one job `gate`, triggers
`push`/`pull_request`/`workflow_dispatch` intact.

**NO-CLAIM.** This makes a verdict *reachable per commit*; it does not make it *read*, and the
unread-red failure is a separate and still-open problem — 51 failures in the last 100 runs with
**no run id or sha cited anywhere in `.beads/issues.jsonl`**. It also costs runner minutes, which
is the trade this row is making explicit rather than hiding.

---

## ⛔ `rch exec` SYNCS THE WORKTREE, SO EVERY REMOTE GREEN IS A **WORKTREE** GREEN (binding, 2026-09-11)

**Rule 8 says `cargo` reads the WORKTREE while a sha names a TREE. THE REMOTE LANE DOES NOT
ESCAPE THAT — it makes it worse, because the lane feels more authoritative.** `rch exec` syncs
the working tree **including untracked files**, so a remote build compiles exactly the thing that
is *not* committed.

```
f1ff868  23:47:36   a path-scoped commit took a CALLER while its CALLEE was UNTRACKED
  git show f1ff868:crates/omp-orchestrator/src/host_precondition.rs   ABSENT FROM THE TREE
  git show f1ff868:.../lib.rs     | grep -c 'pub mod host_precondition'   0
  git show f1ff868:.../resident.rs| grep -c 'use crate::host_precondition' 1   <- swept in
d3d76fe  23:52:22   repaired.   HEAD COULD NOT COMPILE FOR 4m46s.
```

⭐ **AND THE INSTRUMENT THAT SHOULD HAVE CAUGHT IT WAS THE ONE CONCEALING IT.** The author cited
*"the crate is 240 passed / 0 failed / exit=0"* as evidence the sweep was harmless — **a figure
produced by syncing the untracked file HEAD was missing.** It reported its own run as *"re-run
AFTER the sha so it measures HEAD"*; **it measured the sha's worktree neighbourhood, a different
object.** A worktree-syncing remote build is **structurally blind** to a file absent from HEAD.

**THE CHECK, and it must read the COMMITTED TREE, not your checkout:**
```
git archive HEAD | tar -tf - | grep <the-file-you-depend-on>      # 0 = HEAD is broken
```
**AFTER PATH-SCOPING A COMMIT ON A CONTENDED FILE, ASK WHETHER WHAT YOU TOOK REFERENCES ANYTHING
YOUR PATHSPEC COULD NOT TAKE.** A sweep that takes the caller and leaves the callee is a broken
HEAD that **every local and every remote run hides**, because both compile the worktree.

⭐ **SECOND FORM, `FreshCloneP0`'s: for `Cargo.lock` the same defect is a commit that takes a
crate's MANIFEST and leaves its LOCK ROWS.** Invisible for the identical reason — the worktree
resolves fine. That is the `jlb` repair arriving from the opposite direction, and it is why
`cargo metadata --locked` **in a fresh clone** is the only instrument that returned different
answers across all three states.

**NO-CLAIM.** This does not make a remote green worthless — it bounds it. A remote `exit=0` is
evidence about the tree that was SYNCED. To make a claim about a commit, check the commit.

---

## ⛔ A PARTIAL VIEW OF A CHANGE DOES NOT ERR SMALL — IT **INVERTS THE VERDICT**

**`GradePoumgFamily`'s generalisation over three independent instruments in one night, and it is
worth more than any of the three instances:**

|instrument|what it showed|the truth|
|---|---|---|
|`git diff --cached` alone|a **destructive 1,387-line deletion**|a **CONSOLIDATION**, 1072 added against 1387 removed|
|a join corpus containing a pasted whole-tree manifest|**full coverage**, empty complement|**two unowned crates**|
|log greps using needles the code never prints|a **dead code path**|**209 firings of a stale image**|

⭐ **IN ALL THREE THE INSTRUMENT RETURNED A CONFIDENT ANSWER OF THE OPPOSITE SIGN — not a null,
not noise.** That is why *"can this instrument return the other answer"* has to be asked
**before** the measurement rather than after the surprise.

**THE GIT PAIR, stated once: NEITHER `git diff` NOR `git diff --cached` IS A VIEW OF A CHANGE.
Each is a view of ONE SIDE of a boundary, and the pair is the instrument.**
```
git diff            worktree vs INDEX    blind to the STAGED half  -> a peer's `git add` vanishes
git diff --cached   INDEX vs HEAD        blind to the UNSTAGED half -> the compensating edit vanishes
git diff HEAD       worktree vs HEAD     sees BOTH -- use this one
```
**Four agents, including the conductor, published "a 1,387-line orphan deletion" from the
`--cached` half alone.** One of them had already found the sibling defect an hour earlier and
walked into this one anyway, **because a peer's number arrived pre-formed and was quoted rather
than measured** — which is this file's borrowed-claim rule, biting inside a correction.

---

## ⛔ DO NOT PUBLISH A **PERMISSION** AS A **MECHANISM** ("forever" vs "we may not")

**Caught by `GradeUldvuP0` on the conductor, 2026-09-11.** I wrote that five guarded test legs
were *"unmeasurable FOREVER on the workers, since both remedies are closed by CONTABO-OR-BUST."*

⭐ **CONTABO-OR-BUST SAYS BUILDS HAPPEN ON CONTABO RATHER THAN ON JOSHUA'S MAC. IT SAYS NOTHING
ABOUT WHAT MAY BE INSTALLED ON A WORKER.** Putting `br` on a Linux worker's PATH is **worker
provisioning** — not a build on the Mac, not a `--target`, not a cross-build, not any failure
mode the ruling was aimed at. **It is outside an AGENT's bounds, which is a statement about US,
never about THE WORLD.**

**This repo has a monument to that exact substitution:** the STANDING AUTHORIZATION block exists
because the orchestrator told the fleet a freeze blocked all code and routed three panes to
audits **while 131 authorized beads sat claimable.** The recorded remedy is ATTEMPT THE
TRANSITION; the corollary here is:

> **BEFORE WRITING "FOREVER" OR "IMPOSSIBLE", NAME WHO WOULD HAVE TO APPROVE THE REMEDY AND
> CONFIRM THEY REFUSED IT.**

**The cost of getting it wrong is that the CHEAPEST remedy never gets costed**: one binary drop
on a worker — no build, no ruling, no CI lane — versus a whole new `macos-14` job. **A bead
recording the first as impossible never weighs it against the second.**

**The correct form is `TautologicalGuard`'s and it was right before I degraded it: UNMEASURABLE
WITH A NAMED PRECONDITION AND A REMEDY.** *"Requires an operator decision nobody has been asked
for"* — **never** *"impossible."*

⛔ **AND ONE HALF OF A CLAIM MUST NOT INHERIT THE OTHER HALF'S VERDICT.** Of those five legs, the
four reaching `mail_sender_pane_identity` turn on whether they need the **ENV VAR PRESENT** or a
**genuinely LIVE tmux pane** — if the variable, it is fixable in-crate and is not a worker
question at all. **UNREAD. Read the path before classifying it.**

---

## The house commit form when the shared index is dirty (`FreshCloneP0`, adopted 2026-09-11)

**Neither pathspec-scoped nor index-scoped committing is atomic in a single checkout — the shared
mutable thing just moves from the worktree to the index.** This form **removes both from the
path** instead of watching them:

```
BLOB=$(git hash-object -w <your-content>)
GIT_INDEX_FILE=<scratch>/tmp-index git read-tree HEAD
GIT_INDEX_FILE=<scratch>/tmp-index git update-index --cacheinfo 100644,$BLOB,<path>
GIT_INDEX_FILE=<scratch>/tmp-index git diff --cached --numstat     # MUST list only your path
GIT_INDEX_FILE=<scratch>/tmp-index git commit -F <msg>
```
Seeded from HEAD, so **no peer can stage into it**; the content is a blob you hashed, so **the
worktree cannot change under you.**

⛔ **THE RECONCILE STEP IS PART OF THE FORM, NOT AN OPTIONAL TAIL.** The REAL index still holds
the pre-commit blob, so `git status` then shows **your own commit as a staged REVERSAL** and the
next committer silently undoes you. Write the committed content to the worktree and
`git add -- <path>` immediately. ⭐ **Fourth instance of the night's pattern: the remedy creates a
new single-valued view that reads as the opposite of what happened.**

**Choosing between the forms — `git diff HEAD --numstat -- <path>` is the discriminator:**
```
a peer is editing YOUR file        worktree race   -> temp-index form (or index form, bracketed)
the index is dirty, your file clean index sweep    -> `git add -- <p> && git commit -- <p>`
```

---

## ⛔⛔ CI IS THE **ONLY** INSTRUMENT THAT COMPILES THE COMMITTED TREE (binding, 2026-09-11)

**Assemble tonight's four separate findings and they close into one fact nobody had stated:**

```
local `cargo test`              compiles the WORKTREE                    (rule 8)
`rch exec -- cargo test`        SYNCS and compiles the WORKTREE,
                                INCLUDING UNTRACKED FILES                (d5bacc3)
fresh clone + metadata --locked RESOLVES the committed tree. DOES NOT COMPILE IT.
CI  (actions/checkout@v4 at the sha, full history, `cargo test`/`cargo run`)
                                >>> COMPILES HEAD. THE ONLY ONE THAT DOES. <<<
```

⭐ **SO "DOES HEAD COMPILE" IS UNANSWERABLE BY EVERY LOCAL AND REMOTE INSTRUMENT WE HAVE.**
`GradeUldvuP0` reached the same wall from the other side and stated it structurally rather than
as laziness: *"there is currently NO SANCTIONED INSTRUMENT that can compile a fresh clone —
builds are remote-only and `rch exec` syncs the WORKTREE, which is the very surface the bead is
about."*

⛔ **THAT RE-FRAMES `REACHABLE_RED_UNREAD` ENTIRELY. CI IS NOT A REDUNDANT SECOND OPINION — IT IS
THE SOLE ORACLE FOR A PROPERTY NOTHING ELSE CAN MEASURE.** And it had produced **no verdict for
five hours**, because `cancel-in-progress: true` on a `workflow+ref` group let every push kill
the running gate: **14 of 15 cancelled, last real verdict four hours stale.** The fix
(sha-keyed group on `push`) is therefore **load-bearing, not housekeeping** — measured
immediately afterward, run `34566431257` survived **2028s to completion while two later pushes
landed**, and delivered `GATE_RUNNER_FAILING count=16 → 7`.

**OPERATIONAL CONSEQUENCES, all binding:**

- **A green from any local or remote `cargo` run is evidence about the tree that was SYNCED.**
  It is NOT evidence that HEAD compiles, and it is specifically blind to a file you never
  committed — which is how `f1ff868` shipped a HEAD naming `crate::host_precondition` without
  containing it, uncompilable for **4m46s**, while the suite reported `240 passed / 0 failed`.
- **After committing, `git archive HEAD | tar -tf - | grep <file>` is the cheap check** — it
  reads the committed tree and needs no build.
- **A cancelled CI run is not a soft outcome. It destroys the only compile evidence that exists
  for that commit**, which is why the old `cancel-in-progress` comment's reasoning — *a later
  commit's whole-tree verdict strictly covers the earlier one* — was sound **only if the later
  run finishes**, and under a wave it never did.
- **NEVER cite a superseded run.** The oracle is *newest run whose conclusion is
  `success|failure`*, and the reason that oracle was ever needed is this defect.

### ⭐ BUT "DOES HEAD COMPILE **FOR THIS CRATE**" IS LOCALLY ANSWERABLE IN ~90 SECONDS

**`GradePoumgFamily`'s clause, and it is the half that stops four beads waiting on an oracle
that cannot run.** Two steps, and neither alone is sufficient:

```
1. CLOSE THE INPUTS.  Hash EVERY file cargo reads for that compilation unit against
   `git show HEAD:`:
     * THE CRATE'S OWN Cargo.toml        <- the one the first draft OMITTED; see below
     * the crate's sources
     * ALL path deps AND THEIR manifests
     * the workspace manifest
     * the crate's Cargo.lock ENTRY
   All identical => the build's inputs ARE committed content, even inside a worktree
   that is 130 files dirty.
2. PROVE LOAD-BEARINGNESS.  Structural presence is necessary, not sufficient: the dep being
   at HEAD does not prove the green DEPENDS on it. Delete the suspect line, re-run, restore.
```

⛔ **THE FIRST DRAFT OF THIS RULE OMITTED THE CRATE'S OWN `Cargo.toml` AND THEREFORE FAILED ON
THE VERY SPECIMEN IT WAS DERIVED FROM.** `GradeUldvuP0` caught it within minutes of the commit.
**A dependency lives in the crate's own manifest, and that file is not a path dep of itself** —
so the published enumeration would hash sources (identical), path deps (identical), workspace
manifest (identical), lock entry (identical), conclude *"the inputs ARE committed content"*, and
then **compile against an UNCOMMITTED manifest supplying the missing dep.** ⭐ **It failed in the
safe-looking direction on the exact case it was built for** — the `8h` shape again: an
enumeration that cannot see the contaminated input returns "clean".

⛔ **AND THE SHAPE IS LIVE IN THIS TREE, NOT HYPOTHETICAL:**
```
 M crates/contabo-reclaim/Cargo.toml    1 insertion / 5 deletions   UNCOMMITTED
 M crates/contabo-reclaim/src/lib.rs    1072 / 16                   UNCOMMITTED
```
**Anyone committing that `lib.rs` path-scoped without the manifest reproduces `poumg.5`
identically.** Both files are part of the six-file unit recorded in `jwlty`, which is why that
bead insists they move together.

⛔ **AND STEP 1 IS A COMPLETENESS OBLIGATION ON THE READER THAT THE COMPILER DOES NOT CHECK.**
`GradePoumgFamily`, about its own clause: **an input you forget to enumerate is invisible in
exactly the way the untracked callee is — your hashes all match and the answer is confidently
wrong.** ⭐ **This is the `HostRequirement` finding one level out: exhaustive matching over the
set you thought of is not completeness over the set that exists.**

**So the honest form of step 1 is NOT *"the inputs are committed"* but *"THE INPUTS I ENUMERATED
are committed"* — a NO-CLAIM, not a proof.** Name the enumeration in the report or the claim is
unbounded.

**THE ENUMERATION, MEASURED IN THIS REPO (not a hypothetical list):**
```
crate sources · ITS OWN Cargo.toml · its build.rs if present
ALL transitive path deps AND their manifests AND their build.rs
the workspace manifest (incl. [workspace.lints] and [profile.*.package.*], which members
                        INHERIT -- a member's own clean manifest says nothing about them)
.cargo/config.toml      -- injects rustflags, changes resolution, lives in NO crate directory
the crate's Cargo.lock ENTRY
any include_str! / include_bytes! / #[path = …] target

measured 2026-09-11:  build.rs x7  (bead-availability, installer, no-shell-gate,
                                    omp-orchestrator, ompo-doctor, pane-truth, tick-monitor)
                      .cargo/config.toml PRESENT
                      include_str!/include_bytes! sites in 18 files
```
⭐ **An omitted input class fails silently in the PASSING direction**, which is the only reason
to enumerate before anyone needs it.

⭐⭐ **THE METHOD DEMOTED A REAL VERDICT ON ITS FIRST USE, WHICH IS THE STRONGEST ENDORSEMENT IT
COULD HAVE.** `WireGradingPacket` ran step 1 on `-p agent-mail-native` and **it FAILED**: six
uncommitted source files (`client.rs` 9/11, `endpoint.rs` 8/2, `error.rs` 1/1, `journey.rs`
6/24, `oracle.rs` 5/7, `tests/packet.rs` 7/4), and **`-w` proved they are not formatting.** It
recorded its own `exit=0` as **a WORKTREE green, not a HEAD verdict**, and asked for its
acceptance leg to be graded as the weaker true claim.

⭐ **AND IT STILL BOUNDED WHAT SURVIVED, which is the part to copy:** every symbol its test
references is present in HEAD at an identical count (`discovery_sources` 2/2,
`ENV_PRIMARY_TOKEN` 8/8, `resume_from` 1/1, `restored` 1/1, `ORIGIN` 4/4), so the dirty files
neither add nor remove anything its leg depends on. **The classification fix is HEAD-correct;
whether the CRATE compiles at HEAD is unknown.** ⛔ **A crate with uncommitted sources is
UNMEASURABLE AT HEAD BY ANYONE — including whoever tries to attribute its CI `FAIL` row.**

⭐ **STEP 1 ALONE IS WHAT A FRESH CLONE GIVES YOU — RESOLUTION, NOT PROOF. STEP 2 ALONE IS WHAT A
MUTATION GIVES YOU — DEPENDENCY, NOT PROVENANCE. TOGETHER THEY ARE STRICTLY STRONGER THAN
EITHER**, and unlike CI they are available immediately.

### ⭐⭐ STEP 1 IS NOT PASS/FAIL — IT TELLS YOU **WHICH CLAIMS YOUR MEASUREMENT CAN CARRY**

**`GradePoumgFamily`'s ladder, and it is what three agents' independent concessions converge
on.** The drift set is the peer-owned files inside your compilation closure:

```
drift set EMPTY                      -> ABSOLUTE claims available ("this crate passes at HEAD")
drift set NON-EMPTY but BRACKETED    -> DIFFERENTIALS available; absolutes NOT
drift set NON-EMPTY and UNBRACKETED  -> RESOLUTION-STABILITY ONLY
```

⭐ **A CLEAN CLOSURE IS STRICTLY BETTER THAN A BRACKETED DIRTY ONE. Bracketing DETECTS that your
closure moved; emptiness means it CANNOT.** `poumg.5` is the specimen: its 8-input closure was
observed clean at three points spanning the whole grade, **so the common-mode assumption nobody
states does not merely hold — it does not arise.**

⛔ **A MULTI-ARM DIFFERENTIAL SILENTLY ASSUMES A CONSTANT CLOSURE, AND IN A SINGLE CHECKOUT WITH
LIVE PEERS THAT IS PRECISELY THE ASSUMPTION NOBODY STATES.** Verifying a file is PRESENT and
DRIFTING is not verifying it is CONSTANT. One mid-sequence write by its owner and your red and
your green are measurements of **two different closures** — at which point you do not have a
differential, **you have two runs.** It cannot be fixed retroactively, so:

**RULE 5 EXTENSION (binding): SHA THE SUBJECT BEFORE AND AFTER EVERY RUN, *AND* SHA EVERY
DRIFTING FILE IN THE CLOSURE ACROSS THE WHOLE ARM SEQUENCE.** The subject bracket catches a peer
planting in YOUR file; **it does nothing about a peer editing a file your compilation READS.**
Equal across the sequence → common-mode held and the word is earned. Unequal → **the
differential is RETRACTED, not weakened.**

⛔ **AND CLEAN-AT-t1 PLUS CLEAN-AT-t3 IS NOT CLEAN-THROUGHOUT** — a file can be edited and
reverted between observations. Three observations against a fixed blob is a strong **BOUND**,
never a **PROOF**.

⛔ **`[dependencies]` VS `[dev-dependencies]` IS THE POLARITY THAT DECIDES THE CLOSURE, AND A
GREP CANNOT SETTLE IT.** A path edge under `[dependencies]` of a dependency IS in your
compilation unit; that dependency's `[dev-dependencies]` are NOT. Measured: `omp-types` reaches
`kernel-only-operator-hook` through `lifecycle-event`'s `[dependencies]` and **is** in the unit;
`text-structure` enters only through `subprocess-contract`'s `[dev-dependencies]` and **is not**.
⭐ **A symbol-absence grep is about what your code NAMES, not what your compilation READS — and a
`pub use` re-export moves a symbol between files without changing any count.** Walk the
manifests transitively; do not grep.

⛔ **THE MOST INVISIBLE MEMBER OF THE INPUT SET NOW HAS A MEASURED SPECIMEN: a dirty `build.rs`
is COMPILED AND EXECUTED, so it changes generated code while appearing in NO source diff of the
crate under test.** `tick-monitor/build.rs` (6/2, uncommitted) sits in `agent-mail-native`'s
closure. **An input set of `src/**` plus manifests omits it entirely.**

⭐ **AND THE GENERAL FORM OF THE CLOSED-GUARD DEFECT, which is the same shape one level up
(`GradeCatch22`, refuting itself with its own earlier data): A CONTROL THAT WATCHES THE COARSER
OF TWO OUTPUTS IS SINGLE-VALUED WITH RESPECT TO THE FINER ONE.** The guard emits a
classification ROW and a leg VERDICT; every existing leg watched only the verdict, which cannot
move because a closed guard yields a clean skip. **The row is two-valued and nobody was reading
it.**

### ⛔ FOUR DIFF INSTRUMENTS, EACH A FALSE-CLEAN FOR THE NEXT QUESTION OUT

**Measured across one night, and the conductor reached for the loosest and reported its answer
as the strictest one's.**

```
git diff              worktree vs INDEX     blind to the STAGED half   (a peer's `git add` vanishes)
git diff --cached     INDEX vs HEAD         blind to the UNSTAGED half (the compensating edit vanishes)
git diff -w           collapses whitespace  BLIND TO POSITION -- a MOVED identical line survives it
strip-space + sort    collapses both        blind to POSITION *and* INDENTATION
git diff HEAD         worktree vs HEAD      sees staged + unstaged. Use this one for "is it dirty".
`sort` compare        line MULTISET         the true pure-permutation test
```

⭐ **"SURVIVES `-w`" DOES NOT IMPLY "DIFFERS IN CONTENT".** Proven with `cat -A`: an odd
`-use asupersync::Cx;` / `+use asupersync::Cx;` pair was **byte-identical on both sides, `$`
terminator included** — a line that MOVED past its neighbour. `-w` cannot collapse it because the
difference is **POSITION, not whitespace**. A hypothesis of "invisible character or line-ending"
was refuted by demanding the bytes; **asking for `cat -A` cost nothing and settled it.**

**THE THREE-WAY SPLIT OF AN 81-FILE DIRTY SET, both instruments run over the same population:**
```
SUBSTANTIVE         76   real content differences
REORDER + REINDENT  19   same lines modulo indentation -- SOME lines changed (leading whitespace)
PURE PERMUTATION     5   `sort`-identical: no line added, removed or edited
```
**Two agents measured 5 and 24 and both were right** — the strict test asks *"was any line
EDITED"*, the lenient one accepts reorder-plus-reindent. **Reporting the lenient number as the
strict one's overstated a class as behaviourally inert.**

⛔⛔ **AND THE RECONCILIATION IS SHARPER THAN A TWO-WAY SPLIT: THREE AGENTS PRODUCED THREE
DENOMINATORS FOR ONE POPULATION AND ALL THREE WERE DEFENSIBLE — BECAUSE THE BUCKETS OVERLAP.**
```
dirty tracked files                         83    <- not 81
  unreadable (staged deletions)              2    contabo-reclaim/src/{model,probe}.rs
  whitespace-only (`-w` clears entirely)     3
  PURE PERMUTATION (sorted-multiset equal)   4
  CONTENT-DIFFERING                         74
                          3 + 4 + 74 + 2 =  83
```
**`admission-reason/tests/planted_known_bads.rs` is BOTH `-w`-clean AND sorted-identical
(numstat 1/1).** One agent counted it a permutation; another tested `-w` first and filed it
whitespace-only. ⭐ **SAME FILE, SAME DATA, DIFFERENT BUCKET — a PRECEDENCE artifact, not a
disagreement.** That is why 4 ≠ 5.

⭐ **SO THE DENOMINATOR RULE GAINS A CLAUSE: IT IS NOT ENOUGH TO NAME THE FILTER, YOU MUST NAME
THE ORDER THE FILTERS ARE APPLIED IN.** Two honest agents running identical predicates over
identical data produce different counts if their precedence differs, and neither is wrong.

⛔ **AND THE POPULATION IS LIVE — 83, NOT 81. Two files went dirty WHILE WE WERE EACH
MEASURING.** Every figure in this section is a snapshot of a moving set, **including the one
being written as it is written.** ⭐ **A census of a shared checkout MUST carry a timestamp AND a
commit, and MUST be re-derived at close** — a fixed number in a bead body is false within the
hour by the same mechanism that makes a transcribed claim status stale on arrival.

⛔⛔ **AND ONE NOTCH HARDER AGAIN: "DIRTY TRACKED FILES" IS NOT A PREDICATE — IT IS FOUR
PREDICATES WEARING ONE PHRASE.** Five instruments, one population, ONE INSTANT, at `6e45880`:
```
git diff --name-only           (worktree vs INDEX)    82
git diff HEAD --name-only      (worktree vs HEAD)     82
git status --porcelain         (ALL entries)         120
  of which ' M'                                       80
git diff --cached --name-only  (staged)                0
untracked ('??')                                      38
```
**Four agents reported 81, 81, 83 and 84 and NONE disagreed** — four predicates, four times, over
a set that grows while you read it. ⭐ **Two differ by exactly 2 AT THE SAME INSTANT** (porcelain
`' M'` vs `git diff`) **because the `contabo-reclaim` pair is ABSENT FROM DISK**, so it appears
in one enumeration and not the other. The 120-vs-82 gap is untracked files — **which nobody
included and nobody said they excluded.**

**THE FULL FORM OF THE RULE, four clauses:**
```
DECLARE THE DENOMINATOR          how many, out of what
DECLARE THE FILTER PRECEDENCE    one file was both -w-clean AND sorted-equal; its bucket
                                 depends on which filter ran FIRST
DECLARE THE ENUMERATING COMMAND  the phrase hides the instrument
CARRY A TIMESTAMP AND A COMMIT   and RE-DERIVE AT CLOSE
```

⛔ **AND THE SHARPEST SELF-CATCH OF THE NIGHT: AN INSTRUMENT THAT WAS *ACCIDENTALLY CORRECT*,
WHICH IS WORSE THAN WRONG.** An agent's `84` used `git diff --name-only` — worktree vs **INDEX**,
the instrument this file corrects as blind to staged work. It equalled `git diff HEAD` **only
because the index happened to be empty**, and the index was empty **only because the conductor
had unstaged `contabo-reclaim` an hour earlier.** ⭐ **Run thirty minutes sooner it would have
read 82 and HIDDEN THE 1,387-LINE DELETION. Nothing in the output announces that it depended on
someone else's cleanup, so the agent would have had no way to notice.**

### ⛔ A TEST COUNT AND AN EXIT CODE BOTH SAY "SOMETHING REDDENED" AND NEITHER SAYS **WHICH**

**Measured on `9ub39`'s leg 2b, which documents itself as a tripwire:** *"the day such a row
returns, this leg goes RED on the adopted verb."* **The row was put back. IT DID NOT GO RED.**
```
adopted_robot_send_verb_does_not_trigger       ... ok       <- the TARGET. Stayed green.
gate_own_source_is_immune_to_its_own_needles   ... ok
genuine_ntm_spawn_outside_kernel_still_matches ... FAILED   left: 2  right: 1
real_workspace_ledger_balances                 ... FAILED   UNDECLARED_PATTERN "robot-send"
```
⭐ **`15 passed / 2 failed / exit=101` reads as a bite. Pinning the legs BY NAME is the only
reason anyone could see that the TARGET leg stayed green** — the count and the exit code agree
that something reddened and neither names it.

**WHY IT CANNOT FIRE, mechanism rather than adjacency:** the specimen is a `//` line comment, so
`strip_line_comment` blanks it before any needle is matched. **It is a proof that
comment-stripping works, filed under a name promising needle-absence detection.**

⭐ **BUT THE PROTECTION IS REAL AND LIVES ELSEWHERE — this is a MISATTRIBUTED guarantee, not a
missing one.** Re-adding the row IS caught absolutely by `real_workspace_ledger_balances`
(`UNDECLARED_PATTERN … enforced absolutely`). **The debt ledger is the true tripwire; the fix is
two lines of doc comment pointing the claim at it, not a new leg.** Same shape as
`require_idle_grader`'s doc comment claiming a property its call site could not deliver.

---

## ⭐⭐ PIN BEFORE YOU RULE — convert an urgent decision into an unhurried one without deciding it

**The best procedural move of 2026-09-11, and it is the inverse of every self-sealing gate in
this file.** An agent investigating 41 dirty files found they were a **DROPPED STASH** — four
dangling `WIP on main:` commits authored by **Josh**, `git stash list` empty, reflog pruned past
the date, `git fsck` listing the content **unreachable**. ⭐ **The 13:55:49 stash timestamp
matched the cohort's mtime TO THE MINUTE** — a mechanism, not adjacency, and the first
hypothesis about those files that could have returned either answer.

**It created four refs and touched no file:**
```
refs/rluzf-worktree-recovery/wip-{45b0aff,2f0513b,f450aa3,06bbdc0}
git fsck --unreachable for that content -> 0        recovery: git show <ref>:<path>
```
⭐ **ADDITIVE, REVERSIBLE, AND IT DECIDED NOTHING. That is the whole point: it removed the
deadline from a judgement instead of making the judgement under one.** Contrast every gate
recorded above that blocks the repair of the condition it detects.

⛔ **AND IT REFUTED ITS OWN LEADING HYPOTHESIS AGAINST ITS OWN CONVENIENCE.** It expected
already-merged branch material — *safe to revert* — checked `refs/branch-rationalization-backup/*`
and found they carry **the same blob as HEAD** for those paths, so the 8.8 MB archive bundle
does **not** contain the content. **Had it not checked, a discard would have been authorised
against a safety net that does not hold the files.**

### ⛔ A LOCAL REF IS A GC GUARD, NOT A BACKUP

**The conductor ruled *"the pinning means this is no longer urgent"* and that was too strong.**
Refs live only in this `.git/`, are unpushed, and **a fresh clone does not carry custom ref
namespaces** — which matters most here, because a fresh clone is the only clean closure and the
fleet re-measures from clones constantly. **Remedy is one non-destructive command, written
OUTSIDE the repo so THE ONE RULE is untouched:**
```
git bundle create <outside-repo>/<name>.bundle <refs…>   &&   git bundle verify <bundle>
-> "records a complete history"   7,223,461 bytes   sha256 ae1924de37d289f9b0177989…
```
**`git gc` / `git prune` / `git reflog expire` remain FORBIDDEN until the content is
dispositioned — the bundle reduces the blast radius, it does not license the operation.**

### ⭐ THE CLAIM-TIER LADDER PAID OFF IN A DIRECTION NOBODY PLANNED

**`poumg.7` was demoted to *differential only* because its closure contained one dirty file it
could not attribute. That file turned out to be a member of the frozen 41.** ⭐ **Had the grade
published a HEAD verdict, the correction would now be landing on an already-CLOSED bead, against
content nobody may touch to re-measure.** The grader declined it because it could not attribute
it — **and was right for a better reason than it had at the time.**

**THE TIER IS NOT BOOKKEEPING. IT IS WHAT MAKES A CLOSED BEAD STILL TRUE AFTER THE FACTS MOVE.**

### ⛔ AND THE WORKER SEES A DIFFERENT WORKSPACE THAN A CLONE — 91 vs 89

```
regenerated FROM A CLONE OF HEAD                 89 entries
`rch exec -- gate-runner --plan` ON THE WORKER   crates=91, LEDGER_DRIFT x2
```
**Because `rch exec` syncs UNTRACKED files, two untracked crates exist for every remote build
and for no clone.** ⭐ **Regenerating a DERIVED file from the worktree — or from the worker —
bakes those rows in and manufactures the OPPOSITE drift in CI.** Derived artifacts
(`Cargo.lock`, `docs/gate-roster.txt`) **MUST be regenerated from a clone of HEAD.** And while
those crates stay untracked, **a remote `--plan` can never report zero drift: those lines are
CORRECT OUTPUT, not residue.**

### ⛔⛔ THE CLOSURE CHECK HAS ITS OWN BLIND SPOT, OF EXACTLY THE KIND IT EXISTS TO DETECT

**Seventh instance of tonight's shape and the most self-referential. AN UNTRACKED INPUT HAS NO
HEAD BLOB TO COMPARE AGAINST, SO A HASH-AGAINST-HEAD CLOSURE CHECK CANNOT SEE IT AT ALL.** Every
other member of the input set fails loudly when it drifts; **an untracked one is invisible by
construction, because the comparison that would catch it has nothing on the other side.**
Measured:
```
git status --porcelain --untracked-files=all -- crates | grep '^??'   ->  9 entries
  TWO WHOLE CRATES: kernel-only-gate/{Cargo.toml,src/lib.rs,tests/…}
                    omp-host-tool-guard/{Cargo.toml,src/lib.rs}
  plus test files in ack-spine, tick-monitor, undrained-pipe-lint
git show HEAD:crates/ack-spine/tests/properties.rs  ->  DOES NOT EXIST
```
⭐ **AND IT IS WORSE THAN INVISIBLE — IT IS ACTIVELY PRESENT IN EVERY REMOTE BUILD AND ABSENT
FROM EVERY CLONE**, because `members = ["crates/*"]` enrols untracked directories and `rch exec`
ships them. **That is the exact polarity that makes a green un-reproducible.**

**SO STEP 1 NEEDS A SECOND ENUMERATION, NOT A LONGER LIST.** Hashing against HEAD answers *"did
my committed inputs drift"*. It cannot answer *"is there an input with no committed
counterpart"*. **Two questions; only the first was mechanised.** The missing probe is one line
and it returns the other answer:
```
git status --porcelain --untracked-files=all <closure dirs> | grep '^??'
```

⛔ **THIS DEMOTES THE LADDER ITSELF, NOT ANY ONE BEAD. "Empty drift set" was defined as *no
COMMITTED input differs from HEAD*. The correct definition is *no committed input differs from
HEAD **AND** no untracked file is in the closure*.** Every top-rung claim made before this was
made under the weaker definition and **is not entitled to the rung until the second probe runs.**

⭐ **The author of the ladder ran the probe against its own top-rung claim rather than asserting
the gap did not apply** — `poumg.5`'s closure returned **zero** untracked entries, and the crate
declares no `mod`, no `include_str!`, no `include_bytes!`, **so the four files hashed ARE the
crate.** That claim now survives a check nobody had thought to run, rather than resting on an
assumption nobody had noticed making.

**THE GENERAL FORM: an instrument that can only compare against HEAD cannot report the existence
of something HEAD has never seen.** *"Can this instrument return the other answer"* — applied to
the instrument built to ask that question.

### ⛔ A COUNT IS SINGLE-VALUED WITH RESPECT TO "WHICH POPULATION IS LEFT"

**Measured 2026-09-11, and the conductor ran the correct probe, got the answer, and narrated
the opposite.** After a 41-file revert out of 81 dirty, the residue is ~40 **either way**:
```
"the 41 were reverted, 40 remain"     count ~40
"the 40 were reverted, 41 remain"     count ~40      <- OPPOSITE FACT, SAME NUMBER
```
⭐ **THE DISCRIMINATOR IS POPULATION MEMBERSHIP, NOT CARDINALITY:**
```
of the dirty files, how many match a stash ref?   ->  0    (cohort reverted)
                                                  ->  41   (cohort remained)
```
**The two-valued probe had already been run and returned `0`; the prose said the cohort
remained.** The single strongest tell that the cohort was gone sat in the same output: a cohort
member read `worktree == HEAD` with `ref` DIFFERENT — **had it remained dirty it would read
`worktree == ref`.**

**Ninth instance of the night's shape, and the first where the instrument was CORRECT and the
NARRATION was single-valued.** Re-derive membership; never track a number.

### ⛔⛔ A BYTE-IDENTICAL RESTORE IS NOT A NO-OP TO ANY MTIME-BASED INSTRUMENT

**Reverting 41 files touched four under `HOOK_SOURCE_CRATES`, and RESTORING A FILE UPDATES ITS
MTIME EVEN WHEN THE BYTES GO BACK TO HEAD.** `state-wildcard-lint/src/main.rs` jumped to
`00:32:06` against a hook at `00:11:50` and refused the next commit. ⭐ **THE REPAIR OF THE
DIVERGENCE TRIPPED THE GATE THE DIVERGENCE CAUSED.**

⭐ **AND IT IS THE `cp -p` TRAUMA FROM THE OPPOSITE DIRECTION.** There, PRESERVING the
pre-mutation mtime made cargo reuse a mutant rlib and a byte-correct restore tested RED. Here,
ADVANCING the mtime re-armed a freshness gate on a byte-correct restore. **Same fact, two
signs:**
```
cp -p          mtime too OLD   -> stale artifact reused, restore looks BROKEN
any restore    mtime too NEW   -> freshness gate re-arms, commit REFUSED
```
**MTIME IS A SIDE CHANNEL, AND ANY REPAIR THAT TOUCHES FILES PERTURBS EVERY INSTRUMENT READING
IT — INCLUDING THE INSTRUMENTS THE REPAIR EXISTS TO HELP.** Anyone cleaning those five crates
will block the fleet's commits while doing it; that is expected, transient, and must be
announced rather than discovered.

### ⭐ THE REVERT ROUTE WHEN `dcg` REFUSES THE DESTRUCTIVE VERBS

`dcg` refuses `git checkout -- <path>` (`core.git:checkout-ref-discard`) and
`git restore` (`core.git:restore-worktree`). **The sanctioned route uses no destructive git verb
at all:**
```
git cat-file blob HEAD:<path>   ->  write those bytes to <path> with an ordinary file write
```
⭐ **Reading a blob and writing a file is not a git mutation** — it needs no policy exception
and is auditable line by line. **And re-derive the target list immediately before writing rather
than trusting an earlier enumeration**; that is what makes a 41-file revert safe.

### ⛔⛔ THE ARTIFACT WHOSE ONLY JOB IS MAKING DELETION DETECTABLE CANNOT DETECT ITS OWN DELETION

**`docs/gate-roster.txt:4` states its purpose: *"Its ONLY job is to make DELETION
detectable."* Measured 2026-09-11:**
```
roster PRESENT and agreeing    2 LEDGER_DRIFT lines    Remote command finished: exit=0
roster ABSENT                 91 LEDGER_DRIFT lines    Remote command finished: exit=0
typed diagnosis naming the absent roster:  ZERO
```
⭐ ***"Rosters agree"* and *"there is no roster"* ARE THE SAME EXIT CODE.** Mechanism is one
line — `gate-runner/src/main.rs:175` `.unwrap_or_default()` **coerces a read ERROR into the
EMPTY SET.**

⛔ **AND THE CORRECT GUARD ALREADY EXISTS SEVEN LINES AWAY, IN THE SAME FUNCTION, FOR THE OTHER
INPUT:** `main.rs:203-209`, `roster.is_empty()` → `EXIT_EMPTY_ROSTER`, *"an empty gate set is an
ERROR, never a pass."* **Applied to the DERIVED set and not to the FILE.** This file's
anti-vacuity rule, implemented correctly for one input and missing for its sibling. Filed as
`eov8a`, deliberately NOT fixed in the commit that owned the roster's CONTENTS — **contents and
the detector's CONTRACT are different beads.**

### ⛔ AN INSTRUMENT THAT NEVER RAN, CAUGHT ON THE WALL CLOCK

**The first attempt at that control returned *"0 drift lines, 1 typed refusal"* — the OPPOSITE
conclusion — in 0.21 SECONDS with NO `Remote command finished` line.** `rch` had refused, and
the greps were matching **its** refusal text rather than `gate-runner`'s.
⭐ **A refused build EXITS 0, so the MISSING SECOND PROOF LINE is the only tell — and 0.21s for
a remote cargo run is the smell that prompted the check.** Tenth instance of the night's shape
and **the first where the instrument was not WRONG but ABSENT.**

### ⭐ A CLAIM THAT COMPILES NOTHING DOES NOT INHERIT THE COMPILE-CLOSURE TIER

**The ladder governs claims that depend on BUILDING.** A committed artifact's CONTENTS can be
verified by set comparison in a fresh clone, compiling nothing — so **it is exempt BY KIND, not
by tier.** `gate-runner`'s closure is dirty, so every *measurement* of it is bracketed-dirty;
the *roster-contents* claim is still absolute. **State the exemption or someone will demote a
claim that never needed the rung.**

### ⭐ THREE ROADS TO THE TOP RUNG — and only two are reachable on demand

```
LUCK          poumg.5                     its closure was clean because the stash missed it
REPAIR        kernel-only-operator-hook   the cohort revert cleared its one drifting input
CONSTRUCTION  jlb                         measured in a FRESH CLONE, whose drift set CANNOT be
                                          non-empty -- four seconds' work, available all night
```
⭐ **Only REPAIR and CONSTRUCTION are reachable on demand, and ONLY REPAIR ALSO MAKES
*COMPILATION* ABSOLUTES AVAILABLE** — because under CONTABO-OR-BUST **a clone can never
compile**. Before the revert, whether a compilation absolute was available to you depended on
whether an eight-day-old stash happened to miss your crate.

⛔ **AND THE REPAIR DID NOT LIFT EVERY DEMOTION — measured by the agent who went looking for its
own upgrade and reported that it was NOT available.** `-p omp-orchestrator` stays bottom-rung:
four `crates/omp-orchestrator/src/*.rs` files are live agent work in the 09-05+ population, not
cohort. ⭐ **And `omp-orchestrator` sits in the closure of 16+ crates via `no-shell-gate`, so it
is the `claim_strength.rs` shape one crate up — the highest-leverage remaining file set.**

### ⛔⛔ "plain `cp` + `touch`, NEVER `cp -p`" IS NOT A UNIVERSAL RESTORE RULE

**It is the correct rule for ONE instrument and the WRONG one for another, and it was published
as universal. Fifteen restores happened under it in one night.**
```
cargo's stale-rlib trap   `cp -p` PRESERVES the old mtime -> cargo reuses the MUTANT rlib and a
                          byte-identical restore tests RED.      REMEDY: ADVANCE the mtime.
hook_freshness            ADVANCING the mtime RE-ARMS the gate and refuses the next commit.
                          REMEDY WOULD BE: PRESERVE the mtime.
```
**Same shape as Rule 6's polarity and `8h`'s staged blindness: a rule derived against one
instrument, stated without naming which.**

⭐ **THE TWO ARE NOT SYMMETRIC, AND THE ASYMMETRY RESOLVES IT:**
```
wrong mtime for cargo   -> a FALSE RED. A WRONG MEASUREMENT, which propagates into a grade.
wrong mtime for the gate-> a REFUSED COMMIT. A delay, LOUDLY REPORTED, stopping at the person
                           who sees it, clearable with an 8-second stability probe.
```
**SO: ALWAYS ADVANCE — and IF YOUR RESTORE TOUCHED A `HOOK_SOURCE_CRATES` FILE, ANNOUNCE THAT
YOU JUST RE-ARMED THE FRESHNESS GATE.** A false measurement travels; a blocked commit does not.

⭐ **AND THE CLOSURE CHECKER SHOULD SAY SO EVEN THOUGH MTIME IS NOT A CLOSURE MEMBER.** mtime is
not an input to compilation and does not belong in the hash set — **but it IS an input to two
instruments the checker's users depend on.** ⛔ **A report reading `DRIFT=0` while the reader's
next commit is refused by a gate their own restore armed is technically correct and practically
confusing.** One line in the emission — *"N restored files are under `HOOK_SOURCE_CRATES`; the
freshness gate is now armed"* — costs nothing and closes the loop between a repair and the
instrument it perturbs.

### ⭐⭐ TWO DEFECT FAMILIES, TWO REMEDIES — do not treat them as one catalogue

**`GradePoumgFamily`'s separation, made while owning an instance of the second kind.** Ten
defects were catalogued in one night and they are not all the same thing:

```
INSTRUMENT DEFECT (nine of ten)  the probe CANNOT return the other answer.
                                 `git diff` blind to staged · `--cached` blind to unstaged ·
                                 `-w` blind to position · hash-vs-HEAD blind to untracked ·
                                 `grep -c` counting substrings · a roster read coerced to empty
  REMEDY: a better probe, and a negative control proving it can return both answers.

REPORTING DEFECT (one of ten)    the probe returned the RIGHT answer and the NARRATION reached
                                 for a DIFFERENT QUANTITY. `cohort-matching = 0` was on screen
                                 while the prose said the cohort remained.
  REMEDY: STATE THE DISCRIMINATOR YOU USED. A better probe fixes nothing here -- the probe was
          already correct.
```

⛔ **THE GENERAL FORM OF THE SECOND: ANY TIME TWO OPPOSITE FACTS IMPLY SIMILAR COUNTS, THE COUNT
IS NOT A MEASUREMENT OF THE THING YOU CARE ABOUT — no matter how carefully you derived it.**

⭐ **AND A SILENT SELF-CORRECTION IS THE SAME FAMILY.** An agent wrote *"the 81→41 cleanup"* and
later *"the 81→40 cleanup"*, correcting itself without flagging it: **a reader in order sees two
numbers and no notice that one is a retraction.** *"The correction existed and did not reach the
reader"* — the same defect as reporting upward instead of announcing to the fleet. **Announce
your corrections; an unannounced one is how a wrong figure survives.**

### ⭐ THE CI GATE FIX IS PROVEN BY OBSERVATION, NOT BY ARGUMENT

```
BEFORE  14 of the last 15 runs CANCELLED · last real verdict 4 HOURS STALE
AFTER   7 runs in_progress CONCURRENTLY · ZERO cancelled · verdicts completing at 2028s
```
**Each push now gets its own sha-keyed group, so a run survives every later push.** ⭐ **That
matters because CI is the ONLY instrument that compiles the committed tree** — the fix did not
improve a signal, **it restored the only source of one.**

### ⛔⛔ THE WORKER HAS NO `.git` — the remote lane is STRUCTURALLY incapable of a HEAD question

**Argued all night, then emitted by an instrument in production as a TYPED REFUSAL, then
verified independently by the conductor:**
```
RCH_REQUIRE_REMOTE=1 rch exec --job -- sh -c 'test -e .git && echo PRESENT || echo ABSENT; git rev-parse HEAD'
  cwd=/Users/josh/Developer/omp-orchestrator
  .git: ABSENT
  fatal: not a git repository (or any of the parent directories): .git
  Remote command finished: exit=0
```
⭐ **`rch` SYNCS THE WORKTREE, NOT A GIT OBJECT STORE.** So on the build host the
hash-against-HEAD comparison **has nothing on the other side** — the remote lane cannot answer
*"does HEAD compile"* **not because of policy, but because HEAD does not exist there.**

**The closure checker printed it as a typed outcome rather than a wrong answer:**
```
INPUT_CLOSURE_UNMEASURABLE  detail=fatal: invalid object name 'HEAD'
INPUT_CLOSURE_UNMEASURABLE  detail=not a git repository
```
⭐ **THE NEGATIVE CONTROL FIRED ON ITS FIRST REAL RUN, which is the whole reason it exists.** An
instrument without it would have read *"no HEAD blob"* as *"no drift"* and reported a clean
closure on a host that cannot compute one.

**CONSEQUENCE FOR THE PER-CRATE CLAUSE: it holds on a HOST with a real `.git` and is
UNMEASURABLE on a worker.** So the three-question split gains its execution site:
```
per-crate "compiles at HEAD"     LOCAL HOST ONLY -- the worker cannot run probe 1 at all
cross-file tracking completeness clone or CI
whole committed tree compiles    CI ONLY
```

⭐ **AND THE TIER IS A PROPERTY OF THE CLAIM, NOT OF THE CRATE** (`GradeUldvuP0`): the checker
takes `bracketed` as an ARGUMENT and never caches a tier on the verdict, so **the same
measurement yields `DIFFERENTIALS_ONLY` or `RESOLUTION_STABILITY_ONLY` depending on what the
caller actually did** — and a contents-or-resolution claim simply does not call it. ⛔ **But
exempting resolution WHOLESALE would re-open the `poumg.5` hole — a dirty manifest supplying a
missing dep — which is why the manifest and the lock are INSIDE the compile closure and not
treated as metadata.**

### ⭐ THE CONSTANCY AXIS IS THREE DEEP, AND THE WEAKEST RUNG IS THE ONLY POST-HOC ONE

```
empty closure        CANNOT move                                     strongest
hash bracket         DETECTS that it moved between observations
line-number witness  detects a SUBSET of movements, POST HOC, when   weakest -- and the only
                     you took no bracket                             one available AFTER THE FACT
```
**Each narrows the window; NONE closes it.**

**THE WITNESS:** a panic's reported line number, identical across a red, a second red and the
restored green, proves the file did not shift under the sequence — **a mid-sequence edit would
have moved it.** Used to rescue a differential whose author had not bracketed.

⛔ **PRECEDING AND SPANNING BRACKETS LOOK IDENTICAL IN A SUMMARY AND ONLY ONE SURVIVES.**
```
PRECEDING  probe BEFORE, run AFTER          the window between them is INVISIBLE
SPANNING   observe before AND after, runs   the window is DETECTABLE AT THE ENDPOINTS
           strictly inside the interval
```
**One agent's retraction was a PRECEDING bracket; another's surviving top-rung claim was
SPANNING — observed at four heads with the mutations strictly inside.** ⭐ **And its author
called that LUCK TWICE OVER: the closure was clean by luck of which crates the dropped stash
missed, and the bracket spanned by luck of a habit of re-pinning after HEAD moved.** *"Neither
was the method working; the method as written says hash the closure and does not say WHEN."*

⛔ ***"CHECK THE CLOSURE FIRST" IS EXACTLY THE READING THAT PRODUCES THE GAP.*** The clause text
must say SPAN.

**FOUR RUNGS, and the fourth is the recovery instrument for having taken none of the first
three:**
```
empty closure        NO WINDOW AT ALL
spanning bracket     window DETECTABLE at the endpoints -- still permits write-and-revert inside
preceding probe      window it CANNOT SEE
line-number witness  POST HOC, needs NO BASELINE -- works where no bracket was taken
```

⛔ **AND IT MUST CARRY ITS BOUND IN THE SAME SENTENCE OR IT BECOMES THE NEXT INSTANCE. IT PROVES
ONLY: no NET LINE-COUNT CHANGE ABOVE the failing assertion.** Three movements are invisible to
it:
```
an edit BELOW the assertion                              invisible
an edit above adding and removing EQUAL counts           invisible
an edit changing a line's CONTENT without changing count invisible  <- the only one that
  -- including AT the asserting line itself                            corrupts a differential
```
⭐ **The third gap needs a SEPARATE argument, and its author supplied one: the dirty content was
a DOC COMMENT, so it can shift line numbers and cannot shift behaviour.** **Anyone copying the
witness alone would not know the second argument was required, and would hold a constancy claim
resting on nothing.**

### ⛔ THE BRACKET MUST SPAN THE RUNS **AND THE RESTORE**

**Two window defects, both measured, both in the METHOD rather than in anyone's execution:**
- **PROBE-THEN-RUN GAP (~90 s).** An agent announced a top-rung claim on a probe-1 drift of 0;
  a peer's edit landed **between its probe and its first run**. Its enumeration was independently
  verified COMPLETE — **the enumeration was right and the TIMING was wrong.** Those are different
  failures and **only the second is fixable by discipline.**
- **RUN-THEN-RESTORE GAP.** Restores verified by `sha256` + `cmp` against the SUBJECT immediately
  after each arm still leave the CLOSURE free to move during the restore window. ⭐ **A
  differential is only as good as its LAST observation, so a hold that releases at the final RUN
  rather than the final RESTORE leaves the same gap one step later.**

### ⛔ A PLANT ANNOUNCE FORBIDS COMMITTING AND SAYS NOTHING ABOUT **MEASURING**

**Measured 2026-09-11 and it produced a false attribution.** An agent re-verifying its own crate
saw **two consecutive reds at DIFFERENT assertions (`:363` then `:370`) with zero code change of
its own, then green twice**, and attributed it to *"environmental — suspect stale scratch
collision on the worker."*
⭐ **Those are EXACTLY the two assertions a peer's plant arms reddened, one per arm.** The peer
had announced *"DO NOT PATH-SCOPE A COMMIT on this file"* — **and the third party was not
**SO THE ANNOUNCE MUST SAY BOTH: do not COMMIT this path, and do not treat a MEASUREMENT of this
crate as your own result until the restore announce lands.** A live plant in your closure
attributes a peer's mutation to your work, and *"environmental"* is the hypothesis it invites.

⛔ **QUALIFIER, because this attribution is INFERENCE and not proof:** the two reddened
assertions match the peer's two arms exactly, **but no timestamp was captured on the third
party's runs**, and a legitimate commit landing mid-sequence is an equally available explanation
(see below). **The rule stands on the mechanism — a live plant in your closure IS attributable
to you — not on this one attribution.**

### ⭐⭐ A CLOSURE CAN BE INVALIDATED BY **CORRECT WORK BY THE RIGHTFUL OWNER**

**The sharpest form of the window defect, and it emerged from an agent diagnosing its own
retraction better than its first guess.** The file that went dirty in its ~90-second
probe-then-run gap was **not a peer scribbling** — it was `b0ad54a`'s content **arriving**: the
doc fix it had itself requested, landing by the file's author, committed four minutes later.

⭐ **THAT MAKES THE GAP WORSE AS AN ARGUMENT FOR BRACKETING, NOT BETTER: *"nobody is editing my
file"* IS NOT A PRECONDITION ANYONE CAN ESTABLISH BY GOOD BEHAVIOUR.** The thing that moves may
be someone doing exactly the right thing, at exactly the right time, at your request.

⛔ **SO A HOLD-ACROSS-N-RUNS MODE MUST BE FRAMED AS DETECTING *INVALIDATION*, SOURCE-AGNOSTIC —
NEVER AS DETECTING INTERFERENCE.** *"Closure moved between observation k and k+1"* is the honest
reason string. *"A peer edited your closure"* would have been **factually wrong here**, since
the editor was the author landing a requested fix. **A diagnostic that names a culprit where the
mechanism names none is the same overclaim class as every retraction tonight.**

### ⭐⭐ NO AMOUNT OF COORDINATION CAN PROTECT A CLOSURE — THE LADDER IS ABOUT OBSERVABILITY, NOT TRUST

**The closing clause, and it rules out the obvious mitigation.** Plant announces, contention
tables, path holds and the entire queue discipline this fleet built in one night protect against
a peer **SCRIBBLING** in your file. ⛔ **THEY ARE STRUCTURALLY INCAPABLE OF PROTECTING AGAINST A
PEER LANDING CORRECT WORK IN YOUR CLOSURE** — because that peer is doing the right thing, owes
you nothing, and in the measured case **was landing a fix the retracting agent had itself
requested.**

⭐ **SO THE MITIGATION FOR THE PROBE-THEN-RUN GAP IS NOT TIGHTER COORDINATION. IT IS THE
SPANNING BRACKET, FULL STOP. A protocol can make interference RARE; only a MEASUREMENT can make
invalidation DETECTABLE.**

**AND THAT IS WHY THE RUNGS NAME NO CULPRIT:**
```
empty closure        nothing can move, so nothing needs observing
spanning bracket     things may move and YOU WILL SEE IT at the endpoints
preceding probe      things may move and YOU WILL NOT
line-number witness  a SUBSET of movement, after the fact, with no baseline
```
**None of those mentions who is editing, or why.** ⭐ **The source-agnostic reason string —
*"closure moved between observation k and k+1"*, never *"a peer edited your closure"* — is not
politeness. It is THE ONLY PHRASING THAT IS TRUE IN THE CASE THAT ACTUALLY OCCURRED.**

⛔ **AND THE AUTHOR OF THE LADDER RECORDED THAT ITS OWN SURVIVING CLAIM RESTS ON TWO PIECES OF
LUCK:** the closure was clean because the dropped stash happened to miss those crates, and the
bracket spanned because of a habit of re-pinning after HEAD moved — **not because the clause
said to.** *"The clause was incomplete and its author did not notice until someone else paid for
it."* **It stands, and it stands for weaker reasons than it appeared to.**

⛔ **AND EVEN A PURE PERMUTATION IS ONLY PRESUMPTIVELY INERT.** `match` arm order, statements
with side effects, `macro_rules!` definition order, overlapping trait impls, and item order read
by a proc-macro all change behaviour while preserving the line multiset. **"Safe to ignore for
CONTENT-DIFF purposes" is not "safe to ignore for COMPILATION."**

---

## ⭐⭐ TWO MUTATION ARMS SEPARATE ONLY IF ONE RED IS A **STRICT SUBSET** OF THE OTHER

**Measured on `input_closure.rs`'s leg 4, where the first arm proved less than it appeared to:**
```
ARM A  the CONSUMER mutated (`is_clean` ignores the untracked set)   -> 2 reds
ARM B  the PROBE mutated (`untracked_in_closure` forced empty)       -> 1 red, a SUBSET of A's
```
**The leg that stayed green under B is the one whose list is INJECTED.** ⭐ **THE THREE
OUTCOMES AND WHAT EACH MEANS:**
```
B reddens ZERO             the enumeration has NO coverage -- only its CONSUMPTION is tested
B reddens A's FULL SET     the "injected" leg is not injected; the arms are NOT separable
B reddens a STRICT SUBSET  probe AND consumer are BOTH load-bearing        <- the only pass
```
**And the asymmetry must be STRUCTURAL, not lucky:** here `untracked_in_closure` has exactly one
caller, inside `check_crate`, which only the hermetic fixture reaches — **so arm A hits the
consumer both paths share and arm B hits the probe only one path reaches.**

⛔ **THE AUTHOR RAN ARM A FIRST AND IT PROVED ONLY THAT THE CONSUMER HONOURS THE PROBE.** The
split was a reviewer's, pre-registered **from the call graph** before either arm ran. **A single
arm over a probe-plus-consumer pair cannot tell you which half is load-bearing** — the same
single-valued defect as a count that cannot name a population.

## ⛔ `blob == HEAD` IS A VALID RESTORE ORACLE **ONLY IF THE SUBJECT WAS CLEAN PRE-WRITE**

**The primary oracle is `cmp` against YOUR OWN PRE-WRITE BACKUP.** `blob == HEAD` is a
secondary that **coincides only when the file was clean before you planted** — plant into an
already-dirty file and a correct restore reports as a failed one, while a *wrong* restore to
HEAD reports as success.

⛔ **AND THE WAY THIS SURFACED IS A NEW DIRECTION OF THE MEASURE-DURING-PLANT GAP: A PEER
MEASURED THE PLANTER'S LIVE PLANT AND FED IT BACK AS ADVICE ABOUT THE PLANTER'S OWN RESTORE
ORACLE.** It reported the subject dirty pre-plant and warned that `blob == HEAD` would
misreport — **it was reading arm A.** ⭐ **Had the planter accepted it, it would have recorded
ITS OWN SUBJECT AS SOMEONE ELSE'S DIRTY FILE.** The general rule it offered was right and was
adopted; **the specific reading was of a mutation that agent had itself announced minutes
earlier.**

⭐ **THE PLANTER VERIFIED RATHER THAN DEFERRED, AND THAT IS THE WHOLE DEFENCE:** the subject was
clean at SNAP-1, the four dirty siblings are named (`cross_pane_hold`, `jsm_suggest`,
`resident_tick`, `spine_emit`), and it is clean now. **Take the general rule, re-derive the
specific reading.**

## ⛔ "ABSENT FROM CI's FAILING LIST" IS **NOT KNOWN RED**, NOT GREEN

**A fourth verdict class, and it is a BOUND FROM AN EXTERNAL ORACLE'S SILENCE rather than a
measurement of your own.** Attributing the authoritative run's five failing legs to their
targets:
```
census_membership.rs   every_advisory_unreachable_row_is_named_in_the_allowance
                       derivation_did_not_convert_one_blocker_into_forty_three
                       an_allowance_row_for_a_wired_or_absent_crate_is_stale_and_fails
                       the_ratchet_deadline_is_a_real_number_and_not_a_sentiment
gate_wiring_wave3.rs   every_wave_output_has_a_reachable_census_trigger
packet_rendering.rs    ZERO of the five
```
⭐ **That retires a risk by ATTRIBUTION — the target closest to a grader's own bead is not
implicated — and it is still NOT a green claim.** *"Absent from CI's failing list"* is weaker
than *"I ran it"*: **an oracle that names four crates says nothing about the seventeen targets
it did not name individually.** ⛔ **The honest verdict is `NOT KNOWN RED`, and only running it
converts that.**

**AND IT CONFIRMS FROM THE OTHER SIDE THAT `241/0 --lib` AND THE `FAILING` ROW WERE NEVER IN
TENSION:** none of the five is among the env-blocked legs the lib fix addressed. **Two true
figures about two different populations.**

⛔ **THREE OF THE FIVE ARE ABSOLUTE-COUNT RATCHETS** — `derivation_did_not_convert_one_blocker_into_forty_three`
most plainly, with `distinct_label_count_does_not_grow` and `the_unstamped_binary_count_only_falls`
in `no-shell-gate`. **Rule 10 exactly, and now baked into an ASSERTION rather than a probe.**
⭐ **DO NOT RAISE THE CEILING — re-express per-crate or as a ratio, or they are red again next
week by construction.**

## ⛔⛔ A TRUE RULE WITH A FALSE PREMISE — and the premise is the load-bearing half

**The cleanest self-correction of the session, by the agent whose rule it was.** It supplied a
peer with a correct general rule (*a restore oracle must compare against the PRE-WRITE state*)
resting on a measurement that was **its peer's own live plant** — an arm that peer had announced
minutes earlier, in a message it had acknowledged.

⭐ **ITS OWN DIAGNOSIS: *"I asked 'is a peer about to write here' three times before COMMITTING
and did not ask 'is an announced plant live right now' once before PUBLISHING A
MEASUREMENT."*** **The discipline was applied to the write path and never to the read path.**

⛔ **AND THE CONSEQUENCE INVERTS THE INTENT: had the peer accepted it, it would have recorded
ITS OWN PLANT as a peer's uncommitted file.** ⭐ **A true rule delivered on a false premise is
more dangerous than a wrong rule, because the rule survives review and the premise does not get
one.** **Take the general rule; re-derive the specific reading.**

## ⭐⭐ WHEN YOU APPROVE A FIX TO AN ORACLE, **ENUMERATE ITS READERS**

**A repaired oracle that starts telling the truth breaks every consumer that was relying on the
lie.** Measured 2026-09-11, reported by the grader **against its own `APPROVED` verdict**.

`every_wave_output_has_a_reachable_census_trigger` (`tests/gate_wiring_wave3.rs:60`) panics
unless **all eleven** wave outputs are `Reachable`. Its population is the identical list as
`COVERAGE_WAVE_OUTPUT_CRATES` at `lib.rs:536`. **`uldvu`'s accepted, mutation-proved result is
10 of 11** — `kernel-only-operator-hook` deliberately `Unreachable`, the census's one genuine
BUILT ≠ WIRED finding. ⛔ **The test CANNOT PASS while that verdict is correct.**

⭐⭐ **AND THE CAUSATION RUNS THE GOOD WAY: before the repair the predicate was
`Cargo.toml.is_file()`, which returns `Reachable` for ANY crate that exists — so this test
PASSED VACUOUSLY, for precisely the reason `uldvu` was filed.** It was green because the oracle
it consumes **could not return `Unreachable`**. ⭐ **THE REPAIR DID NOT BREAK THE TEST; IT
EXPOSED THAT THE TEST HAD NEVER BEEN ABLE TO FAIL. A TRUE RED REPLACING A FALSE GREEN IS THE
REPAIR WORKING** — so part of this crate's `FAILING` status is a CONSEQUENCE of correct work,
and must not be read as a regression.

⛔ **AND THE FIX MUST NOT RE-CREATE THE DEFECT ONE LAYER OUT.** Either
`kernel-only-operator-hook` gets a real trigger — making 11/11 **TRUE rather than asserted** —
or the assertion is re-expressed to permit a **NAMED, DECLARED** `Unreachable` row. ⭐
***Deleting the assertion, or weakening it to "at least one reachable", rebuilds `uldvu`'s own
defect inside the consumer: a test that cannot fail.*** **Rule 10 with the sign flipped — DO NOT
FIX A TRUE RED BY REMOVING THE THING THAT CAN REPORT IT.** Blast radius is wider than the test:
`decide()` branches on `Unreachable` rows too (`an_unreachable_wave_output_blocks_supervisor_decision`).

⭐ **THE GRADER'S OWN LESSON, stated as discipline rather than apology: *"I graded the
predicate, its arms, its known-bads and its count, and I never asked who CONSUMES the
verdict."*** **Enumerating a repaired oracle's readers belongs in the grading bar alongside
*re-derive every count*.**

**NO-CLAIM carried from the reporter: the analysis is from SOURCE plus CI's failing-test names.
`gate_wiring_wave3` is one of the 17 never-compiled targets and was NOT run; the
vacuously-green claim is derived from the old predicate's TEXT, not measured against the old
tree.** Two agents attributed that test to that file by independent routes — one from CI, one
from the bead — **and only one of them connected it to the repair.**

## ⛔⛔ AN INSTRUMENT YOU **NEVER CONSULTED** — reasoning from a mechanism to a consumer without opening the consumer

**A new class, and it cost a five-peer broadcast and a peer's careful narrowing built on top of
it.** An agent measured a real `cargo` fail-fast truncation **in its own run**, reasoned
correctly about the mechanism, and published it as a **live defect in `gate-runner`** —
recommending a one-flag change. ⛔ **`gate-runner` has passed `--no-fail-fast` all along, and
its source comment NAMES THE EXACT CLAIM AS AN ALREADY-MET REQUIREMENT:**
```
crates/gate-runner/src/main.rs:733   "--no-fail-fast",
crates/gate-runner/src/main.rs:715   "load-bearing and is a measured requirement, not a preference:
                                      without it cargo test STOPS at the first failing target"
```
⭐ **THE DISTINCTION FROM EVERY OTHER SELF-CORRECTION TONIGHT: those were instruments that
COULD NOT RETURN THE OTHER ANSWER. This was an instrument NEVER CONSULTED.** The mechanism was
right and **the SUBJECT was not the thing examined** — the same shape as reading a diff to infer
*who* changed something, and as the `--lib` noun error by the same agent an hour earlier.
**`grep -n 'no-fail-fast' crates/gate-runner/src/main.rs` is ONE COMMAND, skipped because the
inference felt complete.**

⭐ **AND THE CATALOGUE NOW NEEDS TWO CLASSES, because one of them no discipline about OUR tools
can reach:**
```
AUTHORED   instruments we built or chose -- a grep needle, a join corpus, a rev-parse guard,
           an absolute-count ratchet, unwrap_or_default coercing a read error to empty.
           Fixable by our own rigour.
INHERITED  a tool's own control flow. cargo reports the IDENTICAL exit=101 whether your run was
           complete or truncated, and exit=0 whether the crate has 1 target or 17.
           Findable ONLY by cross-checking against a count the tool does not report.
```
⛔ **THE ONLY DISCRIMINATOR FOR THE INHERITED CLASS: count `test result:` LINES against the
crate's TARGET COUNT** (`lib + tests/*.rs + doctests`). **Nothing in your own process is wrong
when this bites**, which is what makes it the hardest kind.

## ⛔⛔ A FIGURE MEASURED UNDER `rch` IS NOT COMPARABLE TO CI FOR TARGET-SHAPED ASSERTIONS

**The live explanation for a 5-vs-11 discrepancy, and it matters here more than anywhere because
EVERY BUILD IN THIS REPO RUNS UNDER `rch`.** `rch` rewrites `CARGO_TARGET_DIR` to
`.rch-target-<worker>-pool-<hash>`. ⭐ **The six extra failures are EXACTLY the target/host-shaped
ones:**
```
target_ownership  4   unowned_target_is_rewritten_with_typed_reason · fleet_wrapper_matches_measured_revision …
target_directory  1   repository_cargo_policy_forces_the_owned_target
sota_preflight    1   retained_jsm_suggest_has_sha_and_nonzero_named_skill
```
⛔ **A rewritten target dir is PRECISELY THE INPUT those tests exist to assert about**, so they
may be **artifacts of the measurement lane rather than defects CI missed.** **Neither figure is
wrong; they are measurements of two different environments.** ⭐ **Label every such figure with
its LANE, and never diff an `rch` run against a CI run for a host-shaped assertion without
resolving the environment first.** Hypothesis with a named mechanism — not converted by reading.

## ⭐⭐ A RULE FIRES ON THE CATEGORY YOU ARE **THINKING IN**, NOT ON THE ACTION YOU ARE **TAKING**

**The sharpest self-diagnosis of the session, from an agent that violated its own rule three
times in one night — each time on a rule it had authored, and each time because the action felt
like a different category:**
```
the commit gate       skipped -- the write felt like BOOKKEEPING
the measurement gate  skipped -- the read felt like VERIFICATION
the plant gate        skipped -- the mutation felt like AUDITING MY OWN EVIDENCE
```
⛔ **"Announce before you WRITE" does not say "announce before you MUTATE-FOR-A-GRADE." A PLANT
IS A PLANT whether its purpose is to test a subject or to test your own prior measurement.**
⭐ **The rule's trigger is the ACTION — a byte written to a shared file — and self-exemption
happens at the moment you classify your own act as belonging to some other category.**
**Check the verb you are about to execute, never the errand you think you are on.**

**AND IT SELF-DISCLOSED: unannounced plant, ~26 s window, restored and proven
(`sha256 == pre-write backup`, `cmp` IDENTICAL, `blob == HEAD`, marker census 0, porcelain
empty) — announced before anyone could find it in a diff.** ⭐ **Disclosure before discovery is
what keeps a violation a data point instead of a trust problem.**

## ⛔⛔ A DEFECT ALREADY FIXED AND DOCUMENTED IN THE TREE IS THE EASIEST THING IN THE REPO TO **REDISCOVER AS LIVE**

**FOUR agents reasoned past `gate-runner/src/main.rs:715`, which carried the lesson, the fix,
AND a measured specimen** (*"251 passed / 55 failed came from 52 of 55 targets while reading as
complete"*). ⭐ **The mechanism was real, the reasoning was sound, and only the SUBJECT was
never opened** — which is why this is harder to catch than being wrong: **nothing in the
argument is defective.**

⛔ **THE CATALOGUE IS THREE CLASSES, and the third is the one that ate an hour:**
```
AUTHORED   an instrument we built or chose      -- fixable by our own rigour
INHERITED  a tool's own control flow            -- findable only by cross-checking a count
UNEXAMINED a consumer we never opened           -- refutable by ONE grep, skipped because the
                                                   inference felt complete
```
⭐ **Before reporting a defect IN a consumer, open the consumer.** **`grep -n <flag> <its
source>` costs one command; a five-peer broadcast of a refuted premise costs an hour of peer
work built on top of it.**

## ⛔⛔ A TRUE RED HAS **TWO** CHEAP FAKE FIXES — WEAKEN THE ASSERTION, OR FALSELY SATISFY THE SUBJECT

**The conductor guarded one and dispatched the other, in the same sentence.** The packet read:
*"give it a real reachable trigger and `every_wave_output_has_a_reachable_census_trigger`
becomes 11/11 TRUE rather than asserted"* — **immediately after** *"do NOT weaken the
assertion."*

⛔ **The census recognises five trigger classes** (manifest caller · `.git/hooks/pre-commit` ·
`[package.metadata.gate]` · workflow entry · scheduler row). ⭐ **Four of them would flip the
row GREEN TODAY while the hook stays unwired — a `[package.metadata.gate]` stanza is FOUR
LINES.** That is a **TRIGGER OF CONVENIENCE**: it makes the assertion pass without making the
claim true. ⭐⭐ **`uldvu`'s own defect arriving from the OPPOSITE SIDE — and it is the CHEAPER
of the two to reach for, which is why guarding only the assertion is half a guard.**
```
weaken the ASSERTION   -> the oracle can no longer report the defect   (guarded, loudly)
satisfy the SUBJECT    -> the oracle reports truthfully about a lie    (unguarded, 4 lines)
```

**RULED: the red STAYS.** ⭐ **`kernel-only-operator-hook` really is BUILT-NOT-WIRED, so the
assertion is TELLING THE TRUTH — a red that correctly reports an unwired crate is not a defect
to clear, it IS the finding.** The admissible resolution is the one already written above: a
**NAMED, DECLARED `Unreachable` row citing its blocker** — never a stanza, never a deletion.

⛔ **AND THE BLOCKER IS REAL AND STRUCTURAL: the built hook DENIES `tmux send-keys`, which is
the only working codex-pane dispatch path** (`ntm --robot-send` refuses codex panes with *"cod
composer not visible"*). **Enabling it today leaves NO codex dispatch at all — the bead
predicted this verbatim: *"or the hook blocks real work and gets disabled wholesale."*** Also
measured: **`hooks_certified.toml` DOES NOT EXIST** — the registry its acceptance names as the
gate is absent, not empty.

⭐ **THE DISPATCHER'S RULE: when you forbid one fake fix, enumerate the others.** A packet that
names one hazard licenses every hazard it did not name, and the unnamed one is always cheaper.

## ⛔⛔ THE VERDICT WORD IS A FIELD TOO — a fix can make the FIELD honest and leave the HEADLINE lying

**Measured 2026-09-11 in the live reaper log, 60 occurrences:**
```
prune-cron PASS mode=apply decision=unknown_fail_closed outcome=apply_error candidates=16
prune apply: tier=unknown target_candidates=16 candidate=37.5GB reclaimable=unknownGB
             applied=0.0GB outcome=apply_error
```
⭐ **The bead reported this path printing `outcome=reclaimed`. It no longer does — that half
WAS fixed.** ⛔ **The success token simply MOVED: `outcome=` became honest and the line still
OPENS with `PASS`.** A fail-closed apply error, **37.5 GB of candidates and 0.0 GB applied**, is
reported hourly as a pass.

⭐ **A grep for the OLD dishonest token returns zero and reads as REPAIRED.** The defect
survived its own fix by relocating one field to the left — **and the field it moved into is the
one every reader scans first.** Outcome census over the live log: `apply_error 60 ·
no_target_candidates 34 · not_attempted 30 · reclaimed 23 · partial_reclaim 2`, so **the reaper
is NOT inert — it is erroring while passing**, which is strictly worse than silent.

⛔ **AND IT INVALIDATES AN ACCEPTANCE WHOSE ASSERTION IS CORRECT.** The bead asks that
`candidates=0` under the floor be an ERROR; the live shape is `candidates=16, applied=0.0GB,
verdict PASS`. ⭐ **The rule that catches TODAY's defect is *"a non-`reclaimed` outcome may not
be reported as PASS"* — the mechanism was right and the SUBJECT MOVED**, which is the third
instance of that shape in one session.

**WHEN AUDITING A "FIXED" HONESTY DEFECT, DIFF THE WHOLE LINE, NOT THE FIELD THE BEAD NAMES.**
A token census keyed on the old spelling cannot see a token that changed position.

## ⭐⭐ A CONTROL MUST NOT SHARE THE DISQUALIFYING PROPERTY WITH THE LANE UNDER TEST

**The sharpest control rule of the session, produced by an agent correcting its own
correction.** It tested *"are these failures `rch`/target-dir artifacts?"* by checking whether
they also fail in CI — **and used `ubuntu-latest` as the "clean host" control.**
```
lane under test   Contabo worker   LINUX
the control       ubuntu-latest    LINUX          <- shares the disqualifying property
the tests         hardcode          DARWIN paths  -- $HOME/.local/bin/cargo,
                  nightly-aarch64-apple-darwin, /Volumes/ZestData, /usr/bin/shasum
```
⛔ **The probe was VALID for the narrow question it was written for and VACUOUS for the
question it then answered.** *"Fails on a clean host too"* is **not** *"fails for a
non-environmental reason"* when **both hosts are Linux and the test hardcodes Darwin.** ⭐
**A control blind to the distinction you are drawing cannot draw it** — and its own summary is
the keeper: *"I wrote 'reading a name is not measuring it', then measured with an instrument
blind to the very distinction I was drawing."*

⭐ **AND THE IMPRESSION "HOST-SHAPED" HID THREE DISTINCT CAUSES:** platform paths
(`target_ownership`), a relocated target dir (`target_directory`), and a repo-relative
`.flywheel/` artifact (`sota_preflight`). **One predicate over three causes is wrong for at
least two of them.**

## ⛔⛔ A CRATE-GRANULAR PRECONDITION CANNOT GATE A TARGET — and building one hides everything else

**Measured refusal, `u3f6q` item 1, discharged under its own item 8** (*"report rather than
widening the gate"*):
```
environment_precondition   production call sites = 1   main.rs:720, inside run_crate()
run_crate()                returns EARLY for the WHOLE CRATE
cargo spawns using --test  ZERO
Invocation::Test(name)     lib.rs:563 -> printed at main.rs:184 as `PLAN invocations=18`
                           ADVISORY OUTPUT ONLY. Nothing executes it.
```
⛔ **So gating two legs gates all eighteen invocations and converts NINE real failures into
silent skips** — `target_ownership` 4 + `census_membership` 4 + `gate_wiring_wave3` 1, the last
being the true-red-replacing-a-false-green. ⭐ **Items 1 and 2 are MUTUALLY UNSATISFIABLE at
this granularity, and that is a property of the runner, not of the hypothesis.**

⭐ **THE CORRECTED DESIGN IS NOT IN THE RUNNER AT ALL: a per-test typed skip, because the test
is the only place with target granularity.** **Before building a gate, measure what granularity
its host can express** — a `PLAN` line listing 18 invocations reads like per-target execution
and is a print statement.

## ⛔⛔ `cargo run` UNDER `rch` IS NOT `cargo test` UNDER `rch` — the FOURTH environment axis

**Measured 2026-09-11 on a run whose gate output was valid and whose exit code was not.**
```
[RCH] RCH-E327 remote compile on contabo-3 SUCCEEDED but returned executables for the WRONG
      PLATFORM: this build targets aarch64-apple-darwin and the retrieved artifact(s) are ELF.
      debug/{contabo-reclaim, dispatch-silence-watch, doctrine-retirement-gate, gate-runner, …}
      The local target directory now holds unrunnable binaries. Treating as a build failure (102).
Remote command finished: exit=0          <- THE GATE's exit, produced ON THE WORKER
rch verdict:              exit=102       <- a POST-HOC verdict on the ARTIFACTS
```
⭐ **The test path CONSUMES RESULTS on the worker; the run path RETRIEVES EXECUTABLES to the
host.** So a `cargo run` invocation trips a platform check **`cargo test` never reaches.** ⛔
**NEVER read a `cargo run -p <crate>` exit code under `rch` as that program's verdict without
separating the gate's `exit=0` from `rch`'s `exit=102`.**

⭐ **AND IT IS A FOURTH CAUSE BEHIND THE ONE "HOST-SHAPED" IMPRESSION**, distinct from the
other three:
```
target-dir relocation   CARGO_TARGET_DIR rewritten by the lane        target_directory
host platform           Darwin paths hardcoded in the test            target_ownership
repo-relative artifact  a .flywheel/ file that may not exist          sota_preflight
artifact platform       ELF retrieved to an arm64 Darwin host         ANY `cargo run` under rch
```
⛔ **This one bites whoever measures a BINARY rather than a TEST**, which is why three sessions
of target-dir reasoning never surfaced it. **It also leaves unrunnable ELF binaries in the
local `.rch-target-*` pool** — harmless, whitelisted for reclaim, and **fatal to anyone whose
next step assumes a runnable local artifact.** The existing rule stands and now has a second
producer: **`file <binary>` BEFORE you run it.**

## ⭐⭐ PROVE "NO ASSERTION WAS WEAKENED" **STRUCTURALLY**, NOT BY GREP

```
git show --stat <sha>   ->   ONE file, 78 insertions, ZERO DELETIONS
```
⭐ **A PURE-INSERTION DIFF CANNOT WEAKEN OR DELETE AN ASSERTION.** The anchored greps agreed,
**and the structural fact does not depend on the regex being right** — which is the whole
difference. A grep proving absence is exposed to `8i`; a zero-deletion stat is not.

⛔ **Every acceptance carrying a "must not weaken, rename or delete" clause should be
discharged this way FIRST**, and only then by naming the assertions. **This is the one place in
the session where a structural oracle strictly dominated a textual one.**

## ⛔ ADDRESS A MUTATION BY **CONTENT**, NEVER BY LINE NUMBER

**Measured: rustfmt reflows moved the target attribute from `:471` at HEAD to `:468` in the
worktree** — three reflows, one a pure line MOVE that `-w` cannot collapse because **the
difference is POSITION.** ⛔ **A line-addressed plant would have landed in the WRONG LEG and
produced a red about something nobody was grading.**

⭐ **In a shared dirty checkout a line number is a transcribed value with the shortest
half-life of any we use** — shorter than a bead count, shorter than a CI verdict. **Anchor on
the text you intend to change.**

## ⛔ AN OVER-NARROW CAPTURE AND A REFUSED BUILD ARE INDISTINGUISHABLE FROM AN EMPTY GREP

**A new false-positive mode of the missing-proof-line rule, found by the agent it fired on.** A
post-restore run returned **no proof lines in 7.9 s** and was read as the refusal tell; it was
**a grep too narrow, not a refusal.** ⭐ **Cost one re-run to establish instead of a wrong
claim to retract.**

⛔ **CAPTURE WIDE AND FILTER AFTER — NEVER FILTER AT CAPTURE.** The rule *"a refused build
exits 0, so the ABSENCE of both proof lines is the tell"* is sound and has exactly this hole:
absence is produced by the instrument as readily as by the subject. **Same family as `8k`,
where a line filter deletes a one-line payload.**

## ⛔⛔ THE TRUNCATION WAS IN MY OWN GREP — the instrument produced the reading, again

**Measured 2026-09-11, and the smoking gun is in the command:**
```
what the conductor ran:   gh run view … | grep -oE 'failing_tests[^|]{0,400}'
                                                                   ^^^^^^^
the real line:            2951 bytes · 45 names · ZERO ellipsis · terminates with ')'
```
⛔ **A 2951-byte line clipped at 400 characters showed SIX names. A crate was dispatched on
those six; when a pane measured 45, the conductor published *"CI's renderer truncates"* and
told the fleet to distrust every per-crate list. THE PRODUCER WAS COMPLETE THE WHOLE TIME.**

⭐ **Not a stale value, not an unexamined consumer — A BOUND TYPED ONE PIPE EARLIER AND NEVER
LOOKED AT AGAIN**, violating *capture wide and filter after* ninety minutes after publishing
it. ⛔ **AND A PEER NEARLY SHIPPED THE INVERSE FROM THE SAME LINE**, clipped by its own
`cut -c1-600`, caught only because the cut landed MID-TOKEN at `fleet_w` rather than at a
boundary. **Two agents truncated one line within ten minutes toward opposite conclusions — that
is a line long enough that every default tool clips it, and nothing in any output says so.**

## ⭐⭐ TRUNCATION IS **ONE-DIRECTIONAL** EVIDENCE — presence claims are immune

> **A clip can HIDE a name. It can never INVENT one.**

⭐ **So every attribution built on a quoted list held, and the only class ever at risk was the
one the conductor doubted — an ABSENCE — which then passed on re-check.** ⛔ **When you discover
your corpus may be clipped, RE-CHECK ONLY THE ABSENCE CLAIMS.** Re-verifying presence claims is
wasted work, and treating all conclusions as equally suspect is how one instrument defect
becomes a general retraction.

## ⭐ AN ENUMERATION THAT SHIPS ITS OWN CARDINALITY CANNOT BE SILENTLY SHORTENED

```
GATE_RUNNER_FAILING count=4 names=<4 items>   SELF-CHECKING -- a short render disagrees with itself
FAIL crate=… failing_tests:<no count>         nothing in the output contradicts a prefix
```
⭐ **The self-checking line is the ONLY CI figure nobody had to re-verify all night**, and the
field without a count ate four panes' reasoning. **It immunises against elision at EVERY layer —
emitter, log, and display — because the disagreement is internal to the line.** ⛔ **It would
not have saved the conductor, whose grep would have clipped the count too** — which is the
honest bound, not a reason to skip it.

## ⛔⛔ AN INHERITED FRAMING SURVIVES DATA THAT REFUTES IT IN THE SAME MESSAGE

**The sharpest self-catch of the cluster.** A pane wrote *"CI's own line ends in an ellipsis"*
and, in the SAME message, reported **45 names parsed from that line** — ⭐ **which it could only
have done if the log contained all 45.** The claim and its refutation were adjacent.

⛔ **It did not notice because it INHERITED THE FRAMING FROM THE DISPATCH rather than deriving
it.** A figure you measure gets checked; **a framing you are handed gets used.** ⭐ **A
dispatcher's error propagates further than a worker's, because the worker's own contradicting
data does not trigger a re-derivation of something it never derived.**

⭐ **And the attribution change is the load-bearing half: an elision in the EMITTER is a gate
defect to FIX; an elision in a DISPLAY LAYER is a CITATION RULE.** Filing against the gate would
have been a bead against a defect that does not exist.

## ⛔⛔ A PATTERN WHOSE **ALPHABET EXCLUDES** A CHARACTER ITS SUBJECT CONTAINS

**The 45-vs-49 gap, closed to the unit.** One parse used `grep -o '[a-z][a-z_]\{12,\}'` — **a
character class with NO DIGITS** — against a corpus containing
`staged_rust_mode_100644_is_clean_and_names_write_time_residual`:
```
the real name          1 token
what the class saw     2 fragments, split by `100644`
plus 3 LABEL tokens    failing_targets · failing_tests · unattributed_target
45 − 1 + 2 + 3 = 49    exact
```
⭐ **Its author named the family: a `rev-parse` that PRINTS instead of failing; a CI control
SHARING the disqualifying property with the lane under test; and now a regex whose alphabet
excludes a character its subject contains.** ⛔ **Three disguises, one shape — A CLASS THAT
CANNOT MATCH WHAT IT IS LOOKING FOR — and each time the tell was never asking what the
instrument COULD NOT SAY.**

⛔⛔ **AND THE UNCOMFORTABLE HALF, VOLUNTEERED: THAT 49 HAD BEEN USED AS A CONTROL.** The
argument *"the same command returns 49 here and 9 there, so 9 is a short list and not a clip"*
rested on a **numerically wrong** control. ⭐ **It survives because the control's JOB was to
show the extractor can return a LONG list, and 45 discharges that as well as 49 — but a control
built to defend a conclusion was itself defective, and only the DIRECTION of the error saved
it.** **State that when it happens; a control that is wrong in the harmless direction is still
wrong.**

## ⛔⛔ STRIP **COMMENTS BEFORE LITERALS** — the ordering is load-bearing

**A detector self-classified TWICE in one file, and the second time is the new mechanism.** Its
table lists accessor spellings AS DATA, so a raw `contains` flagged the detector as
host-dependent. The fix stripped string literals — **and it self-classified again, because an
UNBALANCED DOUBLE QUOTE INSIDE ONE OF ITS OWN COMMENTS put the literal scanner OUT OF PHASE for
the rest of the body**, re-exposing the table as code.

⭐ **Third form of the self-referential checker in one session** — after a needle split so a
checker's own code cannot contain it, and a plant marker that would satisfy the detector it
documents. ⛔ **An author who had read BOTH warnings hit it twice in one file**, which is the
argument for a mechanical rule over vigilance: **comments first, then literals, always.**

## ⛔ AN ANCHORED PATTERN IS DEFEATED BY AN ANSI PREFIX

**`grep -c "^error"` returned 0 on a log carrying FIVE compile errors** — the lines begin with
escape sequences, not with `error`. ⭐ **Identical in shape to the conductor's 400-char bound:
A FILTER TYPED ONE PIPE EARLIER, PRODUCING A PROPERTY THEN ATTRIBUTED TO THE SUBJECT.** One
cost a re-read; the other cost a dispatch. **Same defect, different blast radius** — and both
violated *capture wide and filter after* within the hour of its publication, by two different
authors.

⭐ **AND THE CAUSE-COUNT KEEPS RISING WITHIN ONE FILE: four legs in `target_ownership.rs`, FOUR
distinct remedy codes** — `HOST_SHIM_ABSENT`, `WRONG_HOST_PLATFORM`, `MISSING_PLATFORM_TOOL`,
`VOLUME_NOT_MOUNTED`. ⛔ **A single merged predicate would have named the WRONG ABSENT ARTIFACT
for three of the four.**

## ⛔⛔ TWO MECHANISMS CAN LAND ON THE **SAME INTEGER** — numeric agreement is not mechanism confirmation

**The sharpest instrument finding of the session, self-reported as a near-miss.** One agent
hypothesised the 45-vs-49 gap was the log-line PREFIX and tested it:
```
prefix theory    45 + 4 = 49          4 lowercase tokens from `gate entry one point`   EXACT
digit theory     45 − 1 + 2 + 3 = 49  a class with no digits splits ..._100644_...     EXACT
```
⛔ **Both reproduce perfectly. Only the second is true.** Had the other parse rule not been
declared, the gap would have been "closed" with a confident wrong explanation that any reader
could re-derive.

⭐ **AND THE DISCRIMINATOR WAS ONE MEASUREMENT AWAY AND UNTAKEN: only ONE name in that line
contains a digit**, so the two theories are separable by a single check. **When your
explanation reproduces an observed figure exactly, ask what OTHER mechanism would produce the
same integer, and name the measurement that separates them.**

⛔ **This is *agreement is not independent evidence* at its most dangerous — not two AGENTS
agreeing, but two MECHANISMS agreeing to the unit.** An exact match feels like proof and is
merely a coincidence with good arithmetic.

⭐ **A related reconciliation the same exchange produced: `22` and `20` were never the same
quantity** — `rch` 52 failing / 22 targets against CI 45 / 20, two different SOURCES.
**Carry both figures WITH their sources rather than adjudicating between them.**

## ⛔ A LEGITIMATE ADDITION THAT REDDENS A CENSUS IS THE CENSUS **WORKING**

**Ruled 2026-09-11 when a pane needed a FIFTH `HostRequirement` variant in a file another pane
was mid-census in.** The census pins variants BY NAME, so a new variant reddens direction 2
(*"absent from the census"*) **by construction.**

⛔ **Update the census table in the SAME unit. Do NOT weaken direction 2 to accommodate a
legitimate addition** — that is the true-red-replacing-a-false-green trap, and a census that
tolerates unlisted members is the defect it was built to prevent. ⭐ **A name-pinned census is
SUPPOSED to fail on every addition; that is the difference between it and a count.**

## ⭐⭐ CHOOSE THE MUTATION SITE WITH THE SMALLEST **BLAST RADIUS** THAT STILL EXERCISES THE PROPERTY

**A grader refused a mutation its own bead invited.** Leg 7 said *"re-add an excluded directory
or exclude an included one"* — i.e. **edit the root `Cargo.toml`**, the single most shared build
input in the repo. ⛔ **With four panes compiling concurrently, a mutation there breaks EVERY
peer's build for the window, not the planter's.**

⭐ **It got BOTH axes from `build.rs` alone** — same property, **one-crate blast radius**,
because that file changes only its own crate's generated roster and cannot perturb another
crate's compile. **A plant is a loaded gun; that one was pointed at the fleet.**

**This repo has already paid for the root-manifest hazard twice in one hour:** a `Cargo.toml`
under the `crates/*` glob with no `src/` breaks workspace **LOADING**, so `-p <your-crate>`
cannot dodge it and **every cargo command in the repo fails.** ⛔ **Before planting, ask what
ELSE recompiles — and prefer the site whose failure is confined to the crate under test.**

## ⛔⛔ WHEN THE SUITE IS **RED AT BASELINE**, THE EXIT CODE IS NOT A MUTATION ORACLE

**Stated before the run, which is what makes it discipline rather than an excuse:**
```
baseline                9 passed; 1 failed   exit=101   (an unrelated leg, owned by another bead)
every mutation arm      still exit=101
```
⛔ **A suite verdict CANNOT distinguish an arm from the baseline, and neither can the exit
code — both are 101 throughout.** ⭐ **The discriminator must be the NAMED legs plus the
difference-set TEXT** (`EXTRA [...] / MISSING []` versus `MISSING [...] / EXTRA []`).

⭐ **And say so to your auditors in advance: *"do not read `exit=101` as my arm biting."*** This
is rule 7 — pin the message AND the code — arriving in the case where **the code half is
unavailable entirely**, so the message half carries the whole proof.

⛔ **A third arm may not even produce a `test result:` line: an anti-vacuity arm that empties
the roster ABORTS THE BUILD (`UAD_EMPTY_ROSTER`), so it must be read from the COMPILE output.**
**A mutation whose success is a build failure is invisible to every test-result grep.**

⭐ **AND THE OPPOSITE-SIGN ARM IS THE ONE A SINGLE-ARMED KNOWN-BAD NEVER TESTS:** proving
`EXTRA` fires says nothing about `MISSING`. **Two directions, two arms, two difference-set
texts — or the detector is half-proven.**

## ⭐⭐ DECLARE **BENEFICIARY** INTEREST, NOT ONLY AUTHORSHIP

**A new disclosure class, volunteered unprompted by a grader who was NOT the author:**

> *"I am one of the four panes this change exists to serve — I built a six-leg bead on test
> NAMES because CI would not surface causes, and my own acceptance item then refuted four of
> them. That makes me motivated for this to work, which is a reason to be HARDER on it, not
> softer."*

⭐ **The independence bar this repo enforces is AUTHORSHIP — a different pane, not the
implementer.** ⛔ **It says nothing about a grader who BENEFITS from the change passing**, and
that is a real bias with no existing guard: a grader whose own blocked work is unblocked by the
thing it is grading has an interest in a green verdict.

**The remedy is disclosure plus a stated inversion — *harder, not softer* — and a pre-committed
refusal condition.** ⭐ **Here: *"if arm A's reds are not a proper subset of arm B's, I will say
so and the bead does not close on my say-so."*** **A grader who names in advance the
observation that would make it refuse cannot quietly relax the bar afterwards.**

⛔ **AND IT CAUGHT THE AUTHOR'S SCOPE ERROR IN THE SAME BREATH.** The author verified with
`-p gate-runner --bins` → **26 passed**, and reported that as the unit's verification. The
CRATE is **75 passed across EIGHT target groups**. ⭐ **Not wrong — but a narrower denominator
reported under the wider noun, which is the `--lib`-as-the-crate error by the pane that landed
that rule, for the SECOND time in one session.** **State the target selector beside every test
count, always.**

## ⛔⛔ A SIGNAL PRESENT IN **BOTH** CLASSES CANNOT ATTRIBUTE — presence is not discrimination

**Caught by its own author before publication, on a partition of 52 failing legs.** Twelve were
classified `LANE CONDITION` on the strength of real signal in their output:
```
fatal: ambiguous argument 'HEAD': unknown revision      <- the worker's empty object DB
fixture repos missing .git/hooks/pre-commit
br not on PATH
```
⛔ **Then the cross-check: ALL TWELVE ARE NAMED BY CI TOO — and CI has a real `.git` with a
populated object database, so that condition CANNOT be their cause.** ⭐ **The signal was
present and NON-DISCRIMINATING: it appears in both the class it was supposed to identify and
the class it was supposed to exclude.**

⭐ **Attribution requires a signal that is ABSENT from the other class.** Presence alone
attributes nothing — this is the control rule (*a control must not share the disqualifying
property with the lane under test*) arriving **inside a partition** rather than inside a probe.
⛔ **Had it shipped, twelve legs would have been routed to a worker-provisioning remedy that
cannot fix them.**

**The corrected bucket is `UNCLASSIFIABLE`, and that is a real verdict:** *"both lanes failing
is not one cause."* ⭐ **Naming a bucket unclassifiable is more useful than a confident wrong
partition, because it is the bucket a new instrument can convert.**

⭐ **AND THE METHOD THAT FOUND THE RATCHETS EXTENDS: message SHAPE, not name.** Five became
seven — `every_live_bead_carries_at_least_one_taxonomy_label` (175 unlabelled against a ceiling
of 59) **reads like a correctness check and is a ceiling underneath.** **Two independent panes
found extra ratchets the same way, which argues for the method rather than against either
count.**

## ⭐⭐ PIN A MUTATION ARM BY ITS **CODE EFFECT**, NEVER BY PROSE — my own acceptance admitted two implementations

**`8ems7` item 3 said *"arm B: remove the block gating (`current` defaults to `Some`)"*. That
sentence has TWO faithful implementations and they give OPPOSITE verdicts on the item:**
```
ARM A   capture condition forced false                    { legA }              1 failed
ARM B1  current defaults to Some, header STILL assigns    { antiflood }         1 failed   DISJOINT from A
ARM B2  current defaults to Some, header STOPS assigning  { legA, antiflood }   2 failed   superset of A
```
⛔ **Under B1 the item as written is FALSE — the sets are DISJOINT, not nested — while the
property the item exists to establish is MORE than satisfied.** ⭐ **A grader stopping at B1
would have filed a false `CHANGES_REQUESTED` against a correct unit; stopping at B2 would have
confirmed the author's prose without noticing it admits two implementations.**

**The amendment is one line: state the CODE EFFECT — *"the failures-block header must stop
assigning `current`"* — not the intent.**

⭐⭐ **AND THE RESULT THE ACCEPTANCE DID NOT ASK FOR IS STRICTLY STRONGER: A and B1 are DISJOINT
SINGLETONS. Each leg has a mutation that reddens it ALONE.** **That is independently
load-bearing in BOTH directions — better than the strict subset specified, and no count of reds
could have shown it.** ⛔ **When designing arms, aim for DISJOINT SINGLETONS; accept a strict
subset; reject equal sets.**

⭐ **Item 4 was then discharged BY MUTATION rather than by reading** — defaulting `current` to
`Some` is inert on plain passing output and reddened the anti-flood leg only because the
fixture genuinely carries capture-triggering text outside a failures block. **The measurement
came first and the fixture was read afterwards.** That is how a tautology-class item is
supposed to close.

## ⛔⛔ A TIMESTAMP IS A TRANSCRIBED VALUE WITH AN UNDECLARED TIMEZONE — and the DAG beats the clock

**Measured: `gh`'s `createdAt` is UTC (`07:01:45Z`); `git log` dates carry `-0600`.** Compared
naively, **the same wall-clock instant reads as a FIVE-HOUR GAP** — and nearly produced a
report that a pushed commit had never been pushed.

⭐ **It was caught because the DAG DISAGREED WITH THE CLOCK: `git branch -r --contains` said
`origin/main`, which cannot be true of an unpushed commit.** ⛔ **Prefer an ancestry fact over a
time comparison whenever both are available** — `--contains` and `merge-base --is-ancestor` are
timezone-free and cannot be defeated by a format.

## ⛔ A STALE CI ROW IS NOT EVIDENCE A FIX FAILED — second live specimen in one hour

**`6a0a4ed` is on `origin/main` and ZERO of the last 18 verdict-bearing runs contain it**; the
newest verdict-bearing head predates it by **fourteen minutes.** ⛔ **That run's row still lists
both repaired legs as FAILING, and anyone grepping CI right now will find them red and conclude
the fix failed.** ⭐ **Check `git merge-base --is-ancestor <fix> <headSha>` BEFORE reading the
row** — the same precondition that retired the `LEDGER_DRIFT` claim, now with a specimen where
the stale row reads as a REFUTATION rather than as a leftover.

## ⛔⛔ A PRECONDITION READ AS A STATE EXPIRES BETWEEN THE CHECK AND THE ACTION

**Measured: one staged path on the first probe, ZERO on the next — a peer staged AND committed
inside a single measurement window.** ⛔ **With four panes live the index is a MOVING
QUANTITY.** ⭐ **"Verify the index is clean first" is not a durable fact; EMPTINESS IS AN
INSTANT, NOT A STATE** — the same shape as a drained volume and a `dirty=39` snapshot.
```
git diff --cached --numstat && git commit -F msg     # ONE invocation, no gap
```
**The safety comes from the check being in the SAME BREATH as the commit — never a minute
earlier, and never taken by someone else.**

⛔ **AND THE BROADCAST THAT PROMPTED THIS WAS WRONG A SECOND WAY: a staged deletion that leaves
the INDEX is not GONE — it is UNSTAGED.** `1679 added / 1915 deleted` still live in the
worktree, two files still ` D`. ⭐ **Bare `git commit` is safe from it; `git commit -a`, a
path-scoped commit on that crate, and ANY ARM WHOSE CLOSURE COMPILES IT are not.**
**"Gone from the index" and "restored" are different claims.**

## ⛔⛔ A **CONTAINMENT** TEST CANNOT EXPRESS "THE BUILD'S OWN TARGET DIR"

**A guard written to exclude the relocated lane used `dir.starts_with(repo_root)`.** ⛔ **`rch`
relocates to `<repo>/.rch-target-<worker>-pool-<hash>`, which IS inside the repo — so the guard
reported `MEASURED` on exactly the lane it exists to exclude.** ⭐ **The requirement is the
EXACT path `<repo>/target`, not containment.**

⛔ **A guard that could never fire — this repo's own defect class, committed by the pane fixing
that class.** ⭐ **And it was caught because THE LEG WENT RED AND NAMED THE REASON, not by
reading the predicate.** **A too-permissive predicate is invisible to review and loud to a
known-bad; that asymmetry is the argument for running the arm before trusting the guard.**

## ⭐ A BRACKET IS VALID REGARDLESS OF OWNER — BUT THE OWNER LABEL IS A SEPARATE CLAIM

**Eight dirty files were bracketed as a named peer's live work; they belonged to nobody
identified.** ⭐ **Byte-constant is byte-constant, so the DIFFERENTIAL stands.** ⛔ **But an
UNOWNED-file bracket is a WEAKER risk statement than a known-peer one — because with a named
peer you can ask when they will next write, and with an unowned file nobody has announced
anything.** **State which you have.**

## ⭐⭐ A NEW RULE APPLIES RETROACTIVELY TO YOUR OWN **CLOSED** WORK

**Measured 2026-09-11, and it is the strongest instance of self-application in the session.**
Twenty minutes after a grader closed `9vbcl` `MUTATION-VERIFIED`, the fleet landed the rule
that **a literal pre-fix revert beats a synthetic mutation.** It reopened an arm on its own
closed bead:

> *"`a86befe` is a named fixing commit, so `a86befe^` was available the whole time and I did
> not use it. A synthetic arm proves MY MUTATION is detectable; a literal revert proves THE
> CLAIMED DEFECT was detectable. Those are different claims and I published the weaker one as
> if it settled the stronger."*

⛔ **A closed bead is not a settled question when the BAR moves.** ⭐ **The default instinct —
"it closed under the rules in force at the time" — is defensible and produces a corpus of
grades that silently span two standards.** **Re-running one arm costs minutes; a corpus whose
strength varies by close date cannot be cited as uniform.**

⭐ **AND THE REOPENED ARM IS STRICTLY MORE INFORMATIVE: a single-row synthetic mutation could
not exercise BOTH DIRECTIONS at all.** The literal revert must name `in_workspace_absent_from_ledger`
AND `in_ledger_absent_from_workspace` — **and the grader stated in advance that if it does not
reproduce all three rows, that is a finding against a grade carrying its own name.**

⛔ **It also carried the subject-existence check into the same breath** — `contabo-reclaim` and
`doctrine-retirement-gate` must still be workspace members, **or the post-fix green is the
defect masked by removal of its own subject rather than by the fix.**

## ⭐ WHEN YOUR CHANGE IS AN **ADDITION**, DELETING IT *IS* THE LITERAL REVERT

**A pane got the strong form of the literal-revert rule without designing for it, and only
noticed by DIFFING its plant against `git show HEAD:`** — the delta was an unused `use` line
and comments. ⭐ **Nothing in that delta can reach an assertion, so the red is HEAD's OWN
behaviour rather than a break the planter invented.**

⛔ **But that is a property you must VERIFY, not assume.** **For a NEW guard, the whole file's
prior state IS the pre-fix state — so diff your planted file against `git show HEAD:<path>` and
confirm the delta cannot reach an assertion.** A plant that drifts from the true prior state is
a synthetic arm wearing a literal revert's authority.

## ⭐⭐ A NEW HONESTY TIER: `COHERENT-IN-HEAD ✓` IS NOT `HEAD-COMPILES ✓`

**Measured against the author's own interest, after committing:**
```
git show HEAD:tests/target_directory.rs | grep -c UnrelocatedTargetDir  -> 1   consumer
git show HEAD:src/host_precondition.rs  | grep -c UnrelocatedTargetDir  -> 7   definition
git show HEAD:src/lib.rs | grep -c 'pub mod host_precondition'          -> 1   reachable
```
⭐ **That proves the PAIR SURVIVED THE COMMIT TOGETHER — the broken-HEAD defect, checked on the
committed tree because a worktree grep structurally cannot see it.** ⛔ **It does NOT prove
HEAD COMPILES: ten dirty inputs sat in the compile closure — four uncommitted files inside the
crate and six dirty path deps, NONE of them the author's.**

**The claim ladder, stated in full rather than collapsed:**
```
WIRED ✓   COMMITTED ✓   COHERENT-IN-HEAD ✓   HEAD-COMPILES ✗ (unmeasurable by anyone tonight)
```
⭐ **A green measured over content that is in no tree is a WORKTREE green.** **Say which rung
you reached.**

## ⛔ A MARKER CENSUS OF 0 MEANS TWO DIFFERENT THINGS — the disambiguator is the SHA

**`grep -c <marker>` → 0 is satisfied by NEVER PLANTED and by RESTORED equally.** ⭐ **Only the
sha against the pre-write backup distinguishes them**, and only a timeline (arm landed 07:42,
restored 07:44) narrates it. **Report the sha, not the census, as the restore proof.**

⭐ **AND ONE PANE ACCEPTED A CORRECTION THAT DELETED A RULE FLATTERING ITS OWN DESIGN:**
*"I was reaching for a rule that flattered my own one-arm design, which is the same move as
citing an instrument that agrees with you."* **The true justification was narrower and it took
it.**

## ⭐⭐ TWO POINTS SHOW A DISAGREEMENT; **FOUR POINTS** RULE OUT THE WHOLE FUNCTION CLASS

**The conductor reported a residual from two rows: a worker flagged
`disk_free_below_critical_gb` at 44.2 GB free while an unflagged one sat at 40.0 GB.** A peer
then proposed a hypothesis that would have made the flag CORRECT and inverted that conclusion —
**the floor scales per SLOT**, since each slot holds a target pool (`contabo-1` has 2 slots,
the rest have 4). ⭐ **Any per-slot constant between 11.05 and 20 GB fits both rows.**

⛔ **It then killed its own hypothesis before publishing, by measuring the other two:**
```
contabo-3   49.7 GB   4 slots   NOT flagged
contabo-4   44.2 GB   4 slots   FLAGGED      <- flagged BETWEEN two unflagged hosts
contabo-2   30.2 GB   4 slots   NOT flagged
```
⭐⭐ **A flagged host BETWEEN two unflagged hosts with identical slot counts rules out ANY
MONOTONE THRESHOLD on the reported figure — not merely a uniform one, but the slot-scaled one
it had just proposed.** ⛔ **Two points established that the flag DISAGREES with free space;
four points establish it CANNOT BE A FUNCTION OF free space at all.**

**So `disk_free_below_critical_gb` names a quantity it does not measure** — another mount,
inodes, or a ballast file's presence. ⭐ **That is this session's central defect one layer down
in the tooling: a label asserting what the instrument does not read.** **Reported, never
provisioned — `~/.config/rch/*` is not an agent surface, and `rch capabilities` /
`rch workers list --json` carry NO disk field, which is why nobody can settle it from
outside.**

⛔ **AND THE CONDUCTOR'S OWN TWO FIGURES DIFFERED — `45.3 GB` in the reclaim table, `44.2 GB`
in the residual, minutes apart.** ⭐ **Not an error: a moving quantity needs its INSTANT
attached.** The non-monotonicity survives the 1.1 GB drift, which is what makes it robust.

## ⭐ A GREEN FROM A SCAN WHOSE ELIGIBLE SET IS EMPTY BY CONSTRUCTION — third instance

```
rch gc --dry-run   "would remove 0 dir(s), freeing 0 MB"  x4   WHILE THREE SAT AT 90%+
cause: a 12-HOUR IDLE WINDOW on a fleet that is never idle for 12 hours
```
⭐ **An instrument that cannot return the other answer FOR THE POPULATION IT EXISTS TO
SERVE.** Same shape as the census predicate that could only say `Reachable`, and as ten reapers
reporting `PASS` with `candidates=0` against a filling volume. ⛔ **Three instances in one
session, and each one reported SUCCESS while doing nothing.**

## ⭐⭐ A PANIC **LOCATION** DISCRIMINATES CAUSES ONLY FOR ASSERTIONS THAT TEST A VALUE DIRECTLY

**The best rule of the session, and it invalidated the strongest inference anyone made.**
```rust
umbrella_adapter_dispatch.rs:231:5
assert!(first.status.success(), "first init failed: {}", String::from_utf8_lossy(&first.stderr));
```
⛔ **That is an assert on a SUBPROCESS'S EXIT STATUS: EVERY distinct reason the child can fail
lands on that ONE line.** ⭐ **For that class the location is CONSTANT BY CONSTRUCTION and
carries ZERO cause information — the cause is the child's stderr, interpolated into the panic
MESSAGE.**

**So `same location ⇒ same cause` is INVALID for subprocess-exit asserts, and an overturn that
ran on it was wrong for two-thirds of its rows:**
```
twelve legs by ASSERTION SHAPE:
  subprocess-status  8   location is NOT a cause proxy  -> stay UNCLASSIFIABLE
  direct value       4   location IS evidence           -> move
```
⛔ **FOUR rows moved, not twelve.** ⭐ **QUALIFY BY ASSERTION SHAPE BEFORE USING A LOCATION
MATCH AS EVIDENCE.** A location match on a direct value assert is evidence; on
`assert!(child.status.success(), …)` it proves only **that the child failed, never why.**

⭐⭐ **THREE RUNGS OF THE SAME RULE, ALL MEASURED IN ONE SESSION:**
```
CLASS level     a signal present in both classes cannot attribute
FIELD level     a non-empty 104-155 char detail cannot attribute -- it is all HEADER
LOCATION level  a constant assertion line cannot attribute -- every cause lands there
```
**Each one was a checker hunting the right hazard with a discriminator that could not see
it.**

## ⛔ TWO CORRECTIONS OF THE SAME FIGURE CAN CROSS

**Measured: a pane sent `12 → 8` while the conductor was publishing `12 → 8`, each unaware the
other had landed** — so the second read as *"you are STILL publishing the wide number."* ⭐ **A
broadcast is a snapshot; in a five-pane fleet a correction and its acknowledgement can be in
flight simultaneously.** **Before re-asserting a correction, check whether it already landed —
and when you receive one that looks ignored, consider that it crossed rather than that it was
dismissed.**

## ⭐⭐ A **FILE NAME IS NOT A MEASUREMENT** — the fourth rung

**Disclosed because it nearly produced a fourth wrong number.** A shape test printed
*"25 rows: 0 soft, 7 direct"* — **because the input file believed to hold the DRIFT set
actually held the RATCHET set.** ⭐ **The count `7` printed under a heading saying `25`, and it
was caught ONLY because `7 ≠ 25` was visible on the same line.**

⛔ **Had the two buckets been closer in size, the mislabel would have passed and
*"the DRIFT bucket is entirely robust"* would have been reported from a sample of the WRONG
BUCKET.** The set was rebuilt by subtraction from the full 52 and re-run.

⭐⭐ **THAT COMPLETES FOUR RUNGS OF ONE RULE, ALL MEASURED IN ONE SESSION:**
```
CLASS        a signal present in both classes cannot attribute
FIELD        a non-empty 104-155 char detail cannot attribute -- it is all HEADER
LOCATION     a constant assertion line cannot attribute -- every cause lands there
INPUT LABEL  the name on your input file is not evidence of its contents
```
⛔ **Each was a checker hunting the right hazard with a discriminator that could not see it,
and the fourth is the one no amount of care about the INSTRUMENT protects against — it is
about the SUBJECT you fed it.** **Print a heading AND a count together so a mismatch is
visible on one line; that accident is what caught this.**

## ⭐ CLOSING YOUR OWN NO-CLAIM IS WORTH MORE THAN THE FINDING IT QUALIFIES

**The 25 DRIFT rows had been flagged by their own author as untested under the shape rule —
*"if any rest on subprocess-exit asserts, that bucket is soft too and nobody has measured
it."*** ⭐ **It then measured them: 23 hard, 2 soft, both soft rows NAMED rather than the
bucket discounted wholesale.**
```
FINAL: RATCHET 7 · DRIFT 29 (27 hard + 2 named soft) · rch-ONLY 8 · UNCLASSIFIABLE 8 = 52
```
⭐ **Four successive partitions, the last two self-corrected, and FOUR of its own published
claims demoted by their author.** ⛔ **A NO-CLAIM is a debt, not a disclaimer — the pane that
wrote it is the one best placed to discharge it, and leaving it for "the next thread" is how
a residual becomes permanent.**

## ⭐⭐ A FIX THAT REPLACES ONE PROXY WITH ANOTHER PROXY **ONE RUNG UP**

**The sharpest self-diagnosis of the session, by the grader it cost.** It set out to avoid a
known tautology and landed on its successor:

> *"I retired 'completeness by CARDINALITY' and replaced it with 'completeness by LENGTH' —
> the same move one rung up: a derived quantity standing in for the property nobody checked."*

⛔ **And the length range it cited as evidence of richness — 104-155 chars — is explained
ENTIRELY by test-name plus path length: THE TWO THINGS THAT SCALE WITH LOOKING SUBSTANTIAL.**
⭐ **A proxy that grows with the appearance of substance is the most convincing wrong
discriminator available.** **The real check was one character: `grep -c ':$'` → 59.**

⭐ **Its figures were RIGHT and its INFERENCE was wrong — it measured non-emptiness and length
and concluded content.** **State which you measured and which you concluded; they are
different claims and only the first is a measurement.**

## ⭐ A ONE-DIRECTIONAL CLASSIFIER: SAY WHICH WAY IT CAN ONLY BE WRONG

**The shape test extended to all 45 CI-named legs: 10 SUBPROC / 35 DIRECT.** ⛔ **Its author
declared the 10 a LOWER BOUND, not a count** — the classifier greps a four-line window for four
tokens, so **a helper that unwraps status before asserting, or an assert on stdout reached only
after a spawn, reads as DIRECT.** ⭐ **The soft set can only be LARGER, never smaller.**

⭐ **"Recording that the instrument is one-directional is the whole point, given what it is
measuring"** — a classifier that can only under-report softness is safe to use for *"at least
this many are soft"* and unusable for *"the rest are hard."* **Name the direction of your
instrument's error whenever the conclusion depends on it.**

## ⭐ TWO FIGURES DIFFERING BY A KNOWN CONSTANT ARE THE SAME QUANTITY

```
one pane  111-162   regex KEPT the `detail=` prefix
other     104-155   prefix STRIPPED
162 - 155 == 111 - 104 == 7 == len("detail=")
```
⭐ **Same bytes, same quantity, one known constant apart — NOBODY NEEDS TO ADJUDICATE IT.**
⛔ **The counterpart to the 45-vs-49 case, where two mechanisms produced one integer: here two
figures differ and mean the same thing.** **Reconcile before disputing: a constant offset is
an encoding difference, not a disagreement.**

⭐ **AND TWO INDEPENDENT PATHS REACHED THE SAME RETRACTION** — one from a peer's naming of the
header/message split, one from a negative control drawn from known-present content on the other
lane. **The second is the better instrument and its author did not think to run it. Convergence
by different routes is worth more than either route alone.**

## ⭐⭐ WHEN TWO INSTRUMENTS AGREE **AND SHARE A FAILURE MODE**, THE FIX IS A DIFFERENT FAILURE MODE — NOT A THIRD OF THE SAME KIND

**Two panes classified assertion shapes by grepping a window for subprocess tokens and agreed.
Their agreement lifted nothing, because both ask the same question:** *"is this assert PHRASED
like a subprocess check?"* ⛔ **A helper that unwraps status before asserting evades both
identically.**

⭐⭐ **The fix was a structurally different discriminator, and it is CHEAPER than the one it
replaces:** *"does the enclosing test construct a subprocess AT ALL?"* — **a property no
phrasing can evade.**
```
all 35 DIRECT sites: enclosing test body constructs NO subprocess      35 of 35
DIRECT sites in files with ZERO Command/output/spawn anywhere          18   CERTAIN
DIRECT sites in files that DO spawn somewhere (helper possible)        17   residue
interval: 10 <= subprocess-shaped <= 27  of 45
```
⭐ **Eighteen legs moved from *not proven bad* to *proven good evidence*, and the residue is
NAMED BY FILE.** **Before this, 35 sat in a bucket whose test could not have detected the
error.**

⛔ **AND THE NEW INSTRUMENT DECLARED ITS OWN LIMIT: file-level absence of `Command` cannot rule
out a spawn through an imported helper from another module.** ⭐ **So 18 is a FLOOR on
"certainly good", not a proof of 18 — the one-directional discipline applied to the very
instrument built to fix the one-directional problem.**

**The general form: two instruments that disagree can be reconciled; two that AGREE while
sharing a blind spot produce confidence and no information. Ask what question each one asks,
and if it is the same question, build one that asks a different one.**

## ⛔⛔ A REMOTE WORKER TREE IS A SHARED MUTABLE SUBJECT — announce, restore, and NAME THE HOST

**Protocol gap, disclosed by the pane that created it.** It announced a plant on a LOCAL file
and then wrote `.flywheel/sota-preflight/jsm-suggest.json` **into a shared worker's tree with
no announce at all.**
```
find .flywheel -type f on vmi3549740  ->  0          rch's next sync PRUNED the files
ls -d .flywheel                       ->  .flywheel  the mkdir -p shells SURVIVED
```
⭐ **The window closed by ACCIDENT, not by design.** ⛔ **Between that write and the next sync,
`sota_preflight` would have PASSED for any other pane landing on that host — a FALSE GREEN
manufactured on shared infrastructure by a pane running a remedy proof.**

⛔ **Our entire plant discipline assumed the shared subject lives in this checkout. It does
not.** **Announce before writing to a worker, NAME THE HOST, restore and verify.**

## ⛔ A PER-WORKER FACT PUBLISHED AS A LANE PROPERTY — by an instrument that could not tell workers apart

```
run 1  git show HEAD:<path>  -> fatal: not a git repository   HOST NOT CAPTURED
run 2  ls -d .git            -> .git                          HOST NOT CAPTURED
run 3  ls -d .git  HOST=vmi3549740 -> No such file or directory
```
⛔ **"The Contabo worker tree has no `.git`" was withdrawn because the runs producing the claim
never recorded WHICH WORKER ANSWERED.** ⭐ **At least one lacks it, at least one invocation
found it, and THE LANE WAS NEVER MEASURED — three panes were reasoning on it.**
**CAPTURE THE HOST IN EVERY REMOTE PROBE, or you have a fleet claim built from one machine.**

⭐ **What survived is narrower and real: `.flywheel`'s 44 TRACKED files absent while `.beads/`,
`.cargo/`, `docs/` and `AGENTS.md` were present, and `git check-ignore` EMPTY — the sync is
SELECTIVE, not a dot-directory rule.**

## ⭐⭐ THE VARIANT FORM A TOKEN SCAN DROPS — `.code()` vs `.success()`

```
over 52 sites:  STATUS_SUCCESS 6 · STATUS_CODE 6 · VALUE_direct 40
assert_eq!(output.status.code(), Some(0), "…")   <- subprocess-shaped, INVISIBLE to a success() grep
```
⛔ **HALF the subprocess-shaped sites use the variant spelling.** ⭐ **Same class as a
character class that excludes digits — arriving in the DISCRIMINATOR rather than in the name.**
**This is why a residue cannot be closed by adding more tokens, and why a structural test
beats a lexical one.**

⛔ **AND A CLASSIFIER USED A BACKWARD CONTEXT WINDOW: for a multi-line assert the asserted
expression is on the lines BELOW the panic location.** ⭐ ***A window that looks the wrong way
past the anchor is a capture-wide violation in the SOURCE-READING direction*** — the first
member of that family pointed at source rather than at output.

## ⭐⭐ THREE INSTRUMENTS, THREE POPULATIONS, ONE NUMBER — convergence by DIFFERENT failure modes

```
token scan over 52 rch-lane sites      STATUS_SUCCESS 6 + STATUS_CODE 6 = 12
file-level spawn-absence over 45 CI    residue named by file
bucket-then-shape over 52              8 (in the twelve) + 2 (DRIFT) + 2 (rch-only) = 12
```
⭐ **Same set, reached three ways.** ⛔ **This is the OPPOSITE of the shared-blind-spot case:
two instruments agreeing while asking the same question lift nothing; three agreeing while
asking DIFFERENT questions is real convergence.** **State which question each asked before you
call agreement evidence.**

## ⛔⛔ AN INSTRUMENT THAT IS **ACCIDENTALLY CORRECT** GIVES YOU THE NUMBER AND NONE OF THE UNDERSTANDING

**Disclosed unprompted.** A `.success()` grep is blind to
`assert_eq!(output.status.code(), Some(0))` — **one classifier caught both forms only because
its pattern happened to include a bare `.status` alternative.**

> *"I did not reason about `.code()` and would have missed it had I written the obvious
> pattern."*

⛔ **It produced the right answer and no understanding — and it FAILS SILENTLY THE NEXT TIME IT
IS COPIED**, because the property that saved it was never the property anyone intended. ⭐
**A correct result from an unreasoned instrument is a liability disguised as a confirmation.**
**When your instrument is right, check WHY; if the reason is not the one you designed, say so.**

## ⭐⭐ GRADE YOUR PARTITION BY EVIDENCE HARDNESS — and check whether YOUR OWN claim sits in the residue

**The sharpest self-audit of the session.** Applying the zero-spawn test to its own buckets:
```
23 DIRECT DRIFT rows   CERTAIN 12 · residue 11
 7 RATCHET rows        CERTAIN  5 · residue  2
the 4 rows its own overturn moved:   RESIDUE 4 of 4
```
⛔ **Every row its `12 → 8` overturn rests on sits in a file that DOES spawn — the exact
residue class where a helper could reach the assert and the scan cannot see it.** ⭐ **So the
surviving claim is not *"four rows move"* but *"four rows move ON RESIDUE-TIER EVIDENCE"* — the
weakest tier defined tonight.** **Not withdrawn: a location match on a direct-value assert is
still evidence. But it carries its tier.**

**FINAL HARDNESS-GRADED PARTITION:** `RATCHET 7 (5 certain, 2 residue) · DRIFT 29 (12 certain,
11 residue, 2 named soft) · rch-ONLY 8 · UNCLASSIFIABLE 8 = 52.` ⭐ **A partition that grades
its own rows is the only kind a later reader can use without re-deriving it.**

⭐ **AND THE PAIRING WORTH KEEPING: the emitter took the line ABOVE the message; a classifier
read the lines ABOVE the assertion. BOTH ANCHORED CORRECTLY AND BOTH LOOKED THE WRONG WAY.**

## ⭐⭐ A MUTATION THAT **COULD NOT HAVE PRODUCED THE OTHER ANSWER** — the two-valued check, aimed at the PLANT

**A grader's arm produced a clean, plausible, entirely wrong RED:**
```rust
if requirement != HostRequirement::ShasumTool { return true; }   // WRONG
```
⛔ **For `ShasumTool` that falls THROUGH to the real probe, which returns TRUE on the worker —
so it forced NOTHING, and the leg FAILED where a SKIP was predicted.** ⭐ **A leg that fails
where you predicted a skip is exactly what a real defect looks like.**

> *"The tell was that my arm could not have made the thing I was testing false."*

⭐⭐ **BEFORE READING ANY ARM AS EVIDENCE, ASK WHETHER IT COULD HAVE PRODUCED THE OTHER
ANSWER.** This is the two-valued instrument check — **aimed at the PLANT rather than at the
detector**, which is where nobody had pointed it. ⛔ **The corrected arm
(`== ShasumTool → false, else true`) made the fourth remedy code fire ALONE with its own
remedy text.**

⭐ **AND THE FORMULATION TO KEEP: the other five capture-wide violations DESTROYED BYTES; THIS
ONE DESTROYED A DEGREE OF FREEDOM.** A filter deletes evidence you can re-capture; a mutation
that does not mutate deletes the *possibility* of the other outcome, and leaves a result that
reads as a finding.

## ⭐⭐ RUN THE DECIDING ARM **EVEN AFTER THE AUTHOR CONCEDES**

**The author had ACCEPTED the finding, called its own item unsatisfied, and invited
`CHANGES_REQUESTED`. The grader ran the deciding arm anyway and REFUTED ITS OWN FINDING.**

> *"The concession was premature — mine was the error, not the unit's. I would rather return
> the finding than bank it."*

⛔ **A concession is social evidence and it is the weakest kind in this repo.** ⭐ **Two agents
agreeing that a defect exists is exactly as unfounded as two agreeing it does not — and the
author conceding REMOVES the adversary that would otherwise have caught you.**

⛔ **THE UNDERLYING CONFLATION IS THE REUSABLE HALF: *"unreachable on any host we have"* is NOT
*"unexercised by any instrument."*** ⭐ **An INJECTABLE PROBE collapses the distinction — the
module already shipped `fn present(_) -> bool { true }` / `absent(_) -> false` test doubles, so
the property declared untestable was already testable and nobody looked.** **Before declaring
a property unexercisable, check whether its input is injectable.**

⭐ **And it adopted the host-label amendment RETROACTIVELY AGAINST ITS OWN FIGURES:** 24 remote
runs, host captured only where the tool volunteered it — so *"`shasum` is present"* is measured
on **the workers that answered**, not on *the lane*. **Recorded as a limit on its own grade
rather than discovered later by someone else.**

## ⭐⭐ THREE TIERS OF A CORRECT INSTRUMENT: LUCK · **DEFENSIVE BREADTH** · REASONING

**Two panes caught the same variant form and NEITHER had reasoned about it, in two different
ways — and both said so:**
```
LUCK                a pattern happened to include a bare `.status` alternative that subsumes both
DEFENSIVE BREADTH   four plausible forms enumerated, without reasoning that
                    assert_eq!(status.code(), Some(0)) was a DISTINCT shape needing coverage
REASONING           knew which forms exist and which are excluded
```
⭐ **Defensive breadth is better than luck and worse than reasoning, and it shares luck's
fatal property: it produces the right number and NONE OF THE UNDERSTANDING.** ⛔ ***"The next
person who copies the pattern and trims it to the obvious alternative loses half the set and
gets no warning."***

**The result is confirmed twice over different populations — `6 success + 6 code` over 52
rch-lane sites and `5 + 5` over 45 CI-named sites, the same 50/50 split reached
independently.** ⭐ **So the FINDING is solid and the INSTRUMENTS are not, and those are
separate claims.** **When your pattern is right, say whether you knew why.**

⭐ **AND THE CONCLUSION SURVIVES INTACT: the residue cannot be closed by adding more tokens —
which is why the fix was a discriminator whose FAILURE MODE is different, not one whose
VOCABULARY is longer.**

## ⭐⭐⭐ WHEN TWO MECHANISMS AGREE ON AN INTEGER, COMPARE THE **DECOMPOSITIONS** BEFORE BANKING IT

**The capstone of the session's instrument thread, and it was found by an agent dissatisfied
with a number three panes had already reconciled.**
```
45 CI-named sites   SUCCESS 5 + CODE 5 = 10
52 rch-lane sites   SUCCESS 7 + CODE 5 = 12
52 rch-lane sites   SUCCESS 6 + CODE 6 = 12    <- SAME TOTAL, DIFFERENT SPLIT
```
⭐ **Three agents agreed on one integer and two disagreed about what it was made of. THE TOTAL
WAS THE COINCIDENCE; THE DECOMPOSITION WAS THE MEASUREMENT.**

⛔⛔ **CHASING THE ONE-ROW DIFFERENCE FOUND THE MOST INFORMATIVE ROW OF ALL 52:**
```
empty_staged.rs:376   assert_eq!(output.status.code(), Some(0), …)   <- the rch lane dies HERE
empty_staged.rs:84    assert!(output.status.success(), …)            <- CI dies HERE
```
⭐⭐ **That leg fails at a GENUINELY DIFFERENT ASSERTION on each lane — not a shifted line, two
distinct checks.** **All three counts were CORRECT for their population; `6+6` vs `5+5` vs
`7+5` is ONE LEG BEING TWO DIFFERENT FAILURES.**

⛔ **AND IT UPGRADES A ROW THAT HAD BEEN DISCOUNTED.** Its cross-lane difference had been
called *"weak, because it is itself subprocess-shaped."* ⭐ **It is stronger: the lanes STOP AT
DIFFERENT ASSERTIONS, so they are failing for different reasons — POSITIVE evidence of
divergence, not absence of agreement.** **It is the only row of 52 with affirmative evidence
that the lanes differ, and it was hiding inside a reconciled count.**

⭐ **THREE EARLIER RECONCILIATIONS WERE ALL RESOLVED BY DECOMPOSING** — `45+4=49` as a lane
difference, the length ranges as a 7-byte `detail=` prefix, the crate tallies as a sorted
multiset. ⛔ **This one was banked undecomposed, and that is exactly where the finding was.**

**Agreement on a TOTAL is not agreement. Ask what each side's number is MADE OF, and treat a
matching sum with mismatched parts as a lead rather than a confirmation.**

## ⛔⛔ A PATH-SCOPED COMMIT SWEPT HALF A PEER'S PAIR AND **BROKE HEAD FOR TWENTY MINUTES**

**The conductor's own error, one message after warning a peer about the mirror hazard.**
```
at 9176b51 (path-scoped on main.rs):  main.rs LedgerRead=2   lib.rs LedgerRead=0   HEAD BROKEN
at HEAD after 454328b:                main.rs=2              lib.rs=2              repaired
```
⛔ **`git commit -- <path>` RE-READS THE WORKTREE**, so it took four lines of a peer's
uncommitted CONSUMER while the DEFINITION sat uncommitted in the sibling file. ⭐ **I published
that exact hazard against the RESTORE side minutes earlier and then committed it on the COMMIT
side** — the rule fired on the category I was thinking in, not the action I was taking, for
the fifth time tonight.

⭐⭐ **AND THE STRUCTURAL LESSON, IN THE SHARPER FORM A PEER GAVE IT AFTER I PUBLISHED A LOOSER
ONE.** I wrote that a per-file porcelain check *"certified the file as clean while it was
broken."* ⛔ **That is unfair to the instrument. `git status --porcelain <onefile>` answers
*"is this file modified?"* and it answered CORRECTLY** — indeed that correct answer is what
established `HEAD:` had flipped to post-fix and saved a peer's arm from being a silent no-op.

⭐ **THE DEFECT IS IN THE READING, NOT THE TOOL: `clean` READS AS `healthy` TO THE NEXT
PERSON.** **Two panes measured `main.rs` clean and both were right about that file; the
breakage lived in the RELATION between `main.rs` and `lib.rs`, which no single-file question
was ever asked about.**

⭐⭐ **THE PAIR CHECK IS WORTH MORE POINTED OUTWARD THAN INWARD, and that is the part nobody
did:**
```
git show <sha>:<consumer> | grep -c <symbol>
git show <sha>:<definer>  | grep -c <symbol>
```
**A symbol used on one side and absent on the other is a broken pair AT THAT COMMIT — no
build, no worktree, no ownership required.** ⛔ **Two panes ran exactly this on their OWN
commits, which is why theirs landed coherent, and neither thought to run it on a peer's.**
⭐ ***Your own commit is the one you already understand.*** **Run it on someone else's.**

## ⛔⛔ RETRACTED: "PATH-SCOPED COMMIT IS THE SAFE FORM" IS **HALF** TRUE

**The fleet generalised `git apply --cached` + bare `git commit` as the form that excludes a
concurrent editor — correct, and then over-generalised.** ⛔ **`git commit -- <path>` excludes
the INDEX and STILL RE-READS THE WORKTREE for the path it names: safe against the index, WIDE
OPEN to the worktree.** ⭐ **That is how a commit swept an uncommitted consumer in while its
definition sat uncommitted in a sibling file.** **Second unstated precondition of the session,
beside the addition-is-its-own-revert one.**

## ⛔ THE ADDITION-IS-ITS-OWN-REVERT SHORTCUT HAS AN **UNSTATED PRECONDITION**

**`HEAD:` is your pre-fix blob ONLY WHILE YOUR CHANGE IS UNCOMMITTED.** ⛔ **Once it lands —
or once someone else's path-scoped commit sweeps it in — the SAME COMMAND returns the OPPOSITE
blob.**

⭐ **Measured: an arm announced against `git show HEAD:main.rs` would have "reverted" to the
FIX, observed 5 GREEN, and read as *"my refusal is unnecessary"* — a silent no-op wearing a
result.** **Two peers caught it in the same minute; it was re-sourced from `<fix>^` and
`cmp`-verified byte-identical BEFORE running.** **State the precondition whenever you cite the
shortcut.**

## ⛔ CORRECTION TO MY OWN ANTI-PREFIX CHECK — COUNT `Running` LINES, NOT `test result:` LINES

**I published the inherited-instrument discriminator as *"count `test result:` lines against
the crate's target count."* It is WRONG BY EXACTLY ONE on any crate whose lib compiles
doc-tests.**
```
testable targets from cargo metadata (lib 1 + bin 1 + test 17)   19
"Running …" lines in the log                                     19   <- MATCHES
"test result:" lines                                             20   <- DOES NOT
the extra: "Doc-tests omp_orchestrator" / "running 0 tests"      +1
```
⛔ **`cargo test` emits a `test result:` line for the DOC-TEST target, which is not in the
metadata target list — so the equality as I wrote it is FALSE BY CONSTRUCTION and a grader
enforcing it literally would report a COMPLETE run as a PREFIX.**

⭐ **The PROPERTY is sound and holds; the check named the wrong noun.** **Use `Running` lines
against metadata targets.** ⛔ **Same family as `--lib` returning `0 passed, exit=0` under the
crate's name, and as a character class that excludes digits: the command was right and the
noun did not match its scope — this time in a rule I wrote to prevent exactly that.**

## ⭐ A DELTA MEASURED AGAINST A TREE THAT NO LONGER EXISTS IS NOT A DELTA

**A bead's item 7 predicted a crate would drop from 11 failures to 7; the grader measured 6
and REFUSED to report it as an improvement.** ⛔ **Two commits landed after the bead was
written, and one of the six is a peer's leg still in flight.** ⭐ **What it claimed instead is
what it could prove at its own tree: all four guarded legs read `ok`, and none of the six
failures is one of them.**

**Report the property you can prove at YOUR tree, not the delta against a tree nobody holds.**

## ⭐⭐⭐ DECOMPOSE THE **RESIDUE** TOO — one real finding can hide a plain miss underneath it

**The decomposition rule applied to its own output, five minutes later, and found a SECOND
cause.** The `6+6` vs `7+5` split had been attributed entirely to one leg that fails at a
DIFFERENT assertion on each lane — a real divergence, independently confirmed:
```
ancestry_only_merge_runs_gate_and_preserves_empty_index_refusal
  CI   empty_staged.rs:84    assert!(status.success(), …)        SUCCESS-shaped
  rch  empty_staged.rs:376   assert_eq!(status.code(), Some(0))  CODE-shaped
```
⛔ **But the `.code()` list was ALSO missing `empty_staged.rs:173`** — `one_clean_staged_file_is_clean`,
which fails at the SAME site on BOTH lanes and is no divergence at all. **With it the rch set
is SIX, matching exactly.**

⭐⭐ **So the discrepancy had TWO causes: one genuine finding and one plain miss — and
attributing the whole gap to the finding would have HIDDEN THE MISS BEHIND IT.** ⛔ **Same
trap as the 45-vs-49 gap, where a correct mechanism produced the right integer for the wrong
reason: here a correct FINDING produced the right integer while concealing a second term.**

**A reconciliation that explains the gap is not finished until it explains ALL of the gap.
When your first cause accounts for the difference, check whether it accounts for the WHOLE
difference — a real finding is the most convincing place for an error to hide.**

**FINAL, from source and both logs:** `rch 52 sites: SUCCESS 6 · CODE 6 = 12` ·
`CI 45 sites: SUCCESS 5 · CODE 5 = 10` · the delta is 8 rch-only + 1 CI-only, **plus one leg
that CHANGES SHAPE between lanes.**

## ⛔⛔ A **ZERO-WIDTH WINDOW** — and the NEWER instrument was WORSE than the one it replaced

**The degenerate case of the context-window family, self-reported by the pane that had flagged
the same defect in a peer forty minutes earlier.**
```
173      assert_eq!(                 <- the PANIC SITE (macro start)
174          output.status.code(),   <- the TOKEN being grepped
```
⛔ **It intersected `.code()` TOKEN line numbers against PANIC SITE line numbers BY EXACT
EQUALITY.** ⭐ **For a multi-line assert the token is BELOW the anchor, so the test could only
ever see SINGLE-LINE asserts and was STRUCTURALLY INCAPABLE of finding `:173`.** **A backward
window narrowed to zero.**

⭐⭐ **AND THE SHARPEST PART: ITS ORIGINAL CLASSIFIER READ `l..l+4` AND GOT `:173` RIGHT. The
TOTAL of 12 was always correct; only the DECOMPOSITION — produced by a NEWER instrument built
to refine the answer — was wrong.** ⛔ **A refinement can be a regression. When a second-pass
instrument disagrees with a first-pass one, the newer is not automatically the better; ask
which window each one reads.**

## ⭐⭐ THE SECOND CLAUSE: KEEP DECOMPOSING UNTIL THE RESIDUE IS **ZERO**

> *"I stopped at the first cause because it was interesting. The row left over was the
> unglamorous one."*

⛔ **An INTERESTING cause crowds out a BORING one.** The cross-lane divergence was real,
independently verified, and genuinely the most informative row on the board — **and it
accounted for only ONE of two terms in the gap it was used to explain.** ⭐ **A single
explanation that accounts for a discrepancy is not the explanation accounting for ALL of
it.**

**The rule now reads in full: when two mechanisms agree on an integer, compare the
decompositions — and when the decompositions disagree, KEEP DECOMPOSING UNTIL THE RESIDUE IS
ZERO.** **Its author needed the second clause within five minutes of writing the first.**

## ⭐⭐ A CONTAINMENT CHECK HAS **TWO DIRECTIONS** AND MOST ASSERT ONLY ONE — the watcher unwatched

**Found inside the fix it was written to verify, by the arm that did NOT bite:**
```
remove a requirement from the census GUARDED TABLE  -> 5 passed; 0 failed; exit=0   SILENT NO-OP
remove it from the LEG BODY, table intact           -> census RED, naming BOTH the leg
                                                       and the requirement; leg stays GREEN
```
⛔ **The census asserts `TABLE ⊆ BODY` and NEVER `BODY ⊆ TABLE` at requirement granularity.**
⭐ **Direction 2 catches an unlisted LEG; NOTHING catches an unlisted REQUIREMENT — so a
requirement quietly dropped from the table DISARMS ITS OWN CHECK WITH NO SIGNAL.**

⭐⭐ **The watcher is unwatched in exactly one direction, and that is the very shape the bead
existed to fix.** **A subset assertion must state WHICH direction it checks and what the other
direction would miss** — and the way to find out is the arm that produces a silent no-op, which
most graders discard as a failed mutation rather than reading as a result.

⛔ **AND THE NO-CLAIM PRESSED HARDEST IS THE ONE THAT SURVIVES THE CLOSE: nobody has run those
four legs where their preconditions GENUINELY hold, and nobody can — that needs a Mac and a
local build is forbidden.** ⭐ **Forcing a probe to SAY present does not make the artifacts
EXIST.** **The commit says so and the grader confirmed it rather than quietly discharging
it.**

⭐ **And the 28 deletions in the module half were accounted for BY NAME rather than laundered
by a `254/0` pure-insertion sibling** — six were asserts, and every property survived strictly
stronger: one named pair became all pairs across `ALL`; one accessor became three; a
transcribed total became `ALL.len()`.

## ⛔⛔ "COMMIT FREELY, HOLD THE PUSH" IS **NOT A PER-PANE DISCIPLINE** IN A SHARED CHECKOUT

**A correction to a discipline the conductor adopted and broadcast.** Five panes commit to one
`main` in one checkout. ⛔ **THE NEXT PEER TO PUSH PUBLISHES EVERY PANE'S COMMITS, whether or
not that pane consented or knew.**
```
145383a  6805573  d7e0eb5  454328b   on origin/main -> ALL YES
their authors' reported state         -> "PUSHED ✗" / "not pushed"
the pushes that carried them          -> the conductor's own doctrine pushes
```
⭐ **Two panes reported push state from their own INACTION and both were wrong.** ⛔ **A pane
cannot hold its own push; it can only hold the FLEET's.** **The discipline still worked in
aggregate — it kept 12 uncancellable runs from being 15 — but its unit is the CHECKOUT, not
the agent, and any agent reporting its own push state as an action it controls is reporting a
fiction.**

## ⭐⭐ "I DID NOT PUSH" IS NOT THE SAME CLAIM AS "IT IS NOT PUSHED"

```
git merge-base --is-ancestor <sha> origin/main     <- the only honest push oracle
```
⛔ **Asserting the negative from your own inaction rather than measuring the remote ref** is
the same shape as citing a clean `git status` for a file whose breakage lives in its relation
to another, and as a `.git` claim measured on an unnamed worker. ⭐ **Three instances in one
session by one pane, and this was the cheapest to avoid: ONE COMMAND.**

⛔ **AND IT CONVERTS A KNOWN NEGATIVE INTO AN OPEN QUESTION — the stronger correction.**
`RUNNING ✗` becomes `RUNNING ?`, because a verdict-bearing run containing the commit may exist
by the time anyone reads the claim. ⭐ **A row naming a now-guarded leg as FAILING is evidence
about a tree WITHOUT the guard** — and its disappearance must be verified as the NAMED LEG
VANISHING, never as a bare count dropping.

## ⭐ THE WATCHER IS UNWATCHED IN ONE DIRECTION **AND BLIND TO TWO FILES** — and neither was found by the check

**The pair belongs on the record together.** One census asserts `TABLE ⊆ BODY` and nothing
asserts the reverse at requirement granularity; the same census **cannot see
`target_directory.rs` or `sota_preflight.rs` at all**, so two new variants are guarded by
nothing in that direction.

⭐⭐ **Both facts were found by the panes ADDING to the check, never by the check itself — one
by reading a NON-BITING ARM as a result, the other by recording a GREEN as a DECLARED GAP.**
**Those are the two habits that find a blind spot from inside it.**

## ⛔⛔ A RUN CAN CONTAIN YOUR FIX AND BE EVIDENCE ABOUT **NOTHING** — check the gate PRODUCED OUTPUT

**The worst-shaped trap of the session, caused by the conductor's own broken pair.**
```
newest verdict-bearing run  34577201755 / 9176b517   <- CONTAINS the fix AND the breakage
error[E0432]: unresolved import `gate_runner::LedgerRead`
error[E0425]: cannot find value `EXIT_LEDGER_UNREADABLE`
GATE_RUNNER lines in 415,508 bytes:        0
GATE_RUNNER_FAILURE_CAUSE lines:           0
```
⛔ **`cause lines: 0` at a run containing the cause-capture commit reads EXACTLY like *"the
feature does not work."* It is not. THE GATE CRATE DID NOT COMPILE, so there is no output of
any kind** — not the causes, not the verdicts, not the roster. ⭐ **An absent FEATURE and an
absent BUILD are indistinguishable from the feature's own grep.**

⭐⭐ **`merge-base --is-ancestor` IS NECESSARY AND NOT SUFFICIENT.** Third and worst variant of
the stale-row trap: two earlier ones were runs that PREDATED a fix; **this one POSTDATES the
fix and still shows nothing.** **THE POSITIVE CONTROL FOR EVERY CI READ:**
```
grep -c 'GATE_RUNNER ' <log>     # zero => the gate never ran; every figure is ABSENT-BY-BUILD
```

## ⛔⛔ A SWEEP WHOSE v1 **PASSED THE KNOWN-BROKEN COMMIT** — eleven greens worth nothing

**An outward pair-check sweep over 18 single-file Rust commits returned eleven clean results,
and its author ran the control instead of publishing them.**
```
v1 asked: does this identifier appear ANYWHERE in the crate?
  LedgerRead at 9176b51 -> files-matching = 1 (main.rs ITSELF) -> v1 says "ok"
VERDICT: v1 PASSES the known-broken commit -> VACUOUS for the class it was built for
```
⭐ **The defect is never *"the symbol appears nowhere"*; it is *"the symbol appears only in the
CONSUMER and never in a DEFINER."*** ⛔ **v1 could not express that, so ELEVEN GREENS WOULD
HAVE SHIPPED AS AN AUDIT.** **"Everything is clean" is exactly when to run the control.**

**v2, validated on BOTH controls first:** positive `9176b51` → BROKEN-PAIR on three symbols;
negative `454328b` → ok. **Sweep of 18: one genuine break (mine), six false positives, all
confirmed by reading source — and the three residual mechanisms NAMED: backticked doc text, a
fully-qualified `std::` path with no `use` line, and a string literal spanning a per-line
quote-pairing.** ⭐ **Naming the residual classes matters more than the zero.**

⛔ **AND THE LIMIT ON THE ZERO: only SINGLE-FILE commits were swept, because that is the risk
signature — a multi-file commit can still break a pair if the definition belongs in a THIRD
file it did not touch.** **The claim is "no broken pair among tonight's single-file Rust
commits", never "HEAD is coherent."**

## ⭐⭐ THE PUSH ORACLE IS `git ls-remote`, NOT `origin/main` — a tracking ref is a CACHE

```
git ls-remote origin refs/heads/main      <- what the SERVER has
origin/main                               <- a LOCAL cache, stale in one direction, ahead in the other
```
⭐ **A claim about what is published is a claim about the SERVER.** ⛔ **And the local
instrument was measurably self-contradicting: `.git/refs/remotes/origin/main` carried an mtime
of `02:17` while resolving to a commit made minutes earlier — SO LOOSE-REF MTIME IS NOT A
FRESHNESS SIGNAL.** The pane that found it did not reason from the mtime; **it asked the
remote.**

⛔ **AND THE TWO ERRORS COMBINE BADLY, which is why it is not pedantry.** The guidance attached
to a wrong "unpushed" premise is *"check `merge-base --is-ancestor` before reading a CI row"* —
**the CHECK is right and the PREMISE is wrong.** ⭐ **A reader who believes a commit is
unpushed will read a CONTAINING run as impossible and dismiss a genuine verdict as stale.**
**"Unpushed" and "pushed but unrun" demand the same ancestry check and license OPPOSITE
conclusions when it comes back positive.**

## ⭐ A TIMED-OUT PROBE IS `UNKNOWN`, NOT A NEGATIVE RESULT

**A containment sweep over 40 runs died at 300 s with `error: read: interrupted`.** ⭐ **Its
author reported the half it had PROVEN and explicitly declined the half it had not — rather
than letting an unfinished probe stand in for a negative.**

⛔ **This is the denied-probe rule aimed at a TIMEOUT**, which is the form most likely to be
read as an answer because it looks like the command ran. **Name the clause you did not verify
and the form that would verify it.**

## ⭐⭐ THE RISK SIGNATURE IS **A PATHSPEC NARROWER THAN THE PAIR** — not "single file"

**A correction to the sweep scope committed one hour earlier, from the author of three of its
rows.**
```
3 of the 18 swept commits were MULTI-file, and all three carry BOTH halves of their pair:
  145383a  host_precondition.rs + tests/target_directory.rs
  6805573  host_precondition.rs + tests/sota_preflight.rs
  d7e0eb5  host_precondition.rs + tests/target_ownership.rs
```
⭐⭐ **A two-file commit carrying both halves is STRUCTURALLY INCAPABLE of the broken-pair
defect — the pathspec covered definition and consumer together.** ⛔ **And a single-file commit
is ZERO-risk when its symbol is self-contained and MAXIMUM-risk when its sibling holds the
definition: `9176b51` was single-file PRECISELY BECAUSE the pathspec excluded `lib.rs`.**

⛔ **So "single-file" OVER-includes 15 commits that could not break and UNDER-includes a
multi-file commit whose definition lives in an untouched THIRD file.** ⭐ **The sweep's zero is
not weakened — its JUSTIFICATION is, and the three extras are the safest members of the set.**

⭐ **AND A FOURTH RESIDUAL CLASS, named by the commit's own author against a third party's
attribution:** the stripper must handle **a bare `//` LINE COMMENT with no backticks**, not
only backticked doc text. **Four residual classes, not three** — and the author flagged its own
evidence as weaker than a third party's rather than letting it read as confirmation.

## ⛔⛔ THE ANCESTRY CHECK I TOLD EVERYONE TO RUN WOULD HAVE LICENSED A FALSE READING

**Stated by the pane that had issued the instruction:** run `34577201755` contains `6805573`
**and** `9176b51`, so `merge-base --is-ancestor` comes back **POSITIVE** — ⛔ **and
`grep -c 'GATE_RUNNER '` is ZERO because the gate did not compile.**

⭐ **Its typed skip's ABSENCE from a failing list would have looked like ITS GUARD WORKING,
when nothing was measured at all.** ⛔ ***"That is the exact trap I walked others toward in the
same breath as warning them about stale rows."*** **Third variant of the family tonight, and
the only one where the recommended check actively produces the wrong answer.**

## ⭐⭐⭐ THE CI-EVIDENCE LADDER IS **FOUR RUNGS**, AND EVERY ONE WAS SKIPPED TONIGHT

```
1. PUSHED       git ls-remote origin refs/heads/main   <- NEVER a tracking ref
2. CONTAINED    git merge-base --is-ancestor <sha> <headSha>
3. THE GATE RAN grep -c 'GATE_RUNNER_PLAN_TOTAL' <log>  MUST be nonzero
4. YOUR LEG RAN your crate's row present in that output
```
⛔ **One pane was missing rung 1 by reading a STALE tracking ref, and rungs 3-4 by never
looking.** ⭐ **Ancestry establishes that a run COULD have seen your change; only a positive
control establishes that it DID.**

⛔⛔ **AND THE CIRCULAR CASE IS THE ONE WORTH REMEMBERING: a pane's guards WERE in a
verdict-bearing run — and that run measured nothing, because the conductor's unpaired consumer
took down the runner that would have measured them.** ⭐ **The honest tier is not `RUNNING ✗`
but `PUSHED ✓ · CONTAINED ✓ · MEASURED ✗`**, and *"`RUNNING ✗` published as a modest claim was
an unmeasured guess that happened to be wrong in BOTH directions — the commit ran, and nothing
measured it."*

## ⭐⭐ MY POSITIVE CONTROL IS A **FINISHED** MARKER — and the "robust" substitute would have broken it

**RETRACTED, at source, by the pane that proposed the substitute.** I accepted two objections
to `grep -c 'GATE_RUNNER '` and BOTH WERE WRONG FOR THE SAME REASON — nobody had asked WHERE
each token is printed:
```
:264  let observed = run_crate(...)                 <- THE RUN LOOP
:372  let report = build_report_scoped(...)         <- AFTER the sweep
:375  print!("{rendered}");                         <- the ONLY source of "GATE_RUNNER crates="
:208  println!("GATE_RUNNER_PLAN crates=…")         <- EARLY, plan branch
:219  println!("GATE_RUNNER_PLAN_TOTAL …")          <- EARLY, plan branch
```
⛔ **Objection 1 — *"the census is emitted EARLY, so it proves the gate STARTED not
FINISHED"* — IS FALSE. It is printed at `:375` from a report built at `:372`, downstream of
the run loop at `:264`.** ⭐ **The needle proves the sweep RAN TO COMPLETION**, and the
author's own comment at `:383` says so: *"Written AFTER the measurement and removed BEFORE it,
so absence means 'no verdict was produced' rather than 'an older verdict is still lying
around'."*

⛔⛔ **Objection 2 — replace it with `GATE_RUNNER_PLAN_TOTAL` because that is
*"unconditional"* — WOULD HAVE INTRODUCED THE VERY WEAKNESS OBJECTION 1 FEARED.** That token
is emitted at `:219`, **inside the early plan branch, BEFORE the run loop.** ⭐ ***"My 'robust'
substitute is a started-not-finished marker; the original is a finished marker."***
**"Unconditional" was never the property that matters — "DOWNSTREAM OF THE MEASUREMENT" is.**

⭐⭐ **AND THE TRAILING SPACE DOES TWO JOBS AT ONCE, which is why both objections died on it:
it excludes `GATE_RUNNER_FAILURE_CAUSE` (non-circular — never use the feature under test to
prove the gate ran) AND it excludes the plan-phase lines (post-sweep).** ⛔ **Do not widen it
and do not substitute it.**

**THE PAIR STANDS, for a different reason than first given:**
```
grep -c 'GATE_RUNNER ' <log>   nonzero -> compiled, ran, AND COMPLETED THE SWEEP
grep -c 'could not compile'    zero    -> distinguishes a dead BUILD from a dead RUN
```
⭐ **Two needles, two failure modes — not "started vs finished" but "BUILT vs PRODUCED A
VERDICT."** **And rung 4 of the ladder gets stronger with it: if the sweep completed, your
crate's row being ABSENT is a real negative rather than an ambiguity.**

⛔ **THE LESSON UNDER ALL THREE OF US: we argued about a control from a TOKEN CENSUS and none
of us read WHERE THE TOKENS ARE PRINTED.** ⭐ **A control's strength is a property of its
EMISSION SITE, not of its spelling** — which is the correlate law again, with *token frequency*
standing in for *position in the program*.

## ⛔⛔ MY PROSE CONTRADICTED MY OWN OUTPUT IN THE SAME TURN — and a peer inherited the prose

**The conductor's error, and the data refuting it was on screen when the claim was written.**
```
my own command printed:   contains 9176b51  YES
                          contains 6805573  no      <- RIGHT THERE
my prose then told that commit's author:  "the answer turns out to be
                                           'ran, and measured nothing'"
```
⛔ **That tier belongs to a DIFFERENT commit — one genuinely an ancestor of the blinded run.
`6805573` has never been in a verdict-bearing run AT ALL.** ⭐ **Two different tiers, and I
handed over the dramatic one.**

⛔⛔ **AND THE PEER THEN RESTATED IT AS ITS OWN READING — the sixth instance of a transcribed
value, and the first with NO INSTRUMENT IN IT.** Every other tonight was a tool clipping bytes
or a window looking the wrong way. ⭐ ***"The value was transcribed from PROSE, which is the
cheapest possible place to lose provenance"*** — and its own diagnosis names why: *"I found
its shape more interesting than my own situation, and adopted its conclusion without
re-deriving the ancestry for my commit."*

⭐⭐ **AN INTERESTING TIER IS ADOPTED FASTER THAN A BORING ONE**, exactly as an interesting
cause crowds out a boring one. **Two commits adjacent in a message is not evidence that they
share a state.**

## ⛔⛔ RETRACTED: "A STRUCTURAL NEGATIVE BEATS A SEARCH" — I CANONIZED A CORRELATE AS AN IMPOSSIBILITY

**I published this as a ⭐⭐ rule and three panes endorsed it:** *"the newest verdict-bearing
head is an ANCESTOR of my commit, so no run in flight COULD contain it — the difference
between 0 found and 0 possible."* ⛔ **IT IS FALSE. Measured, twelve of twelve:**
```
in-flight runs containing the commit:  12 of 12
adbd4ce9 · 082c57c5 · de1e62b8 · 01c8765b · ba84d3b1 · 373451ad
6c2c5748 · 605f2a98 · 35e2377f · 936d0898 · 947bfbbc · 87d7c0a9
```
⭐ **The MEASURED half stands exactly: `0` VERDICT-BEARING runs contain it, properly measured
over 35.** ⛔ **The INFERENCE bolted onto it does not: a fact about the newest VERDICT-BEARING
head bounds nothing about IN-FLIGHT heads, which are NEWER BY CONSTRUCTION — they are the runs
that started after the completed ones.** **"No verdict exists" is TRUE. "None is pending" is
FALSE.**

⭐⭐⭐ **AND IT IS THE SESSION'S OWN PATTERN WEARING THE STRONGEST LABEL WE HAVE.** The cheap
observable was *"newest verdict-bearing head"*; the property was *"any in-flight head"* — and
the substitution was not merely made, it was **PROMOTED TO `0 possible` OVER `0 found`**, the
label reserved for exhaustive structural proof. ⛔ ***A CORRELATE DRESSED AS A STRUCTURAL
IMPOSSIBILITY IS WORSE THAN A CORRELATE, BECAUSE THE LABEL INSTRUCTS THE NEXT READER TO STOP
CHECKING.*** **It cost one command to falsify.**

**What survives: `0 found` over a stated population, with the population named.** **"Structural"
is a claim about the ORDER OF THE GRAPH and must quantify over the set you actually mean —
here, in-flight heads, not completed ones.**

## ⛔ A TIMED-OUT PROBE MAY BE A **BUDGET** ARTIFACT, NOT A HARD QUESTION

**The sweep that returned `UNKNOWN` twice — correctly reported as `UNKNOWN` both times — was
pure API budget:**
```
gh run list --limit 30/40   -> times out at 300s
gh run list --limit 12      -> returns in 3.9 SECONDS
```
⛔ **Two `UNKNOWN`s were an artifact of asking for more rows than the API would deliver in the
window, not evidence about CI — and the whole in-flight picture was four seconds away the
entire time.** ⭐ **Reporting `UNKNOWN` was right; ACCEPTING it was premature. Check the budget
before accepting the unknown** — a probe that cannot finish is not necessarily a hard question.

⭐⭐ **AND THE 9-vs-12 DELTA IS THE LESSON ARRIVING ON ITS OWN SUBJECT.** One pane measured
**9 of 12** in flight; two others measured **12 of 12** minutes later. ⛔ **The difference is
TIME — three runs started between the probes — which is the moving-quantity rule landing on
the very figure being used to argue that nothing was moving.** ⭐ **Both correct at their
instants, decomposed rather than left as a bare disagreement.**

⭐⭐ **THE SYMMETRIC HALF OF THE BUDGET RULE, and it is what actually happened here: AN
`UNKNOWN` THAT IS CHEAP TO RESOLVE AND LEFT STANDING BECOMES THE GAP SOMEONE ELSE FILLS WITH
AN INFERENCE.** ⛔ **Two honestly-reported `UNKNOWN`s sat on the board; the space they left was
filled by a structural-sounding guess that three panes then endorsed.** **Reporting `UNKNOWN`
is a duty; LEAVING one cheap to resolve is a hazard.**

⛔ **AND THE LIVENESS NEEDLE IS NOT PORTABLE TO A `cargo test` LOG OF ITS OWN CRATE:** nine
occurrences of `GATE_RUNNER crates=` live inside `ci_citation.rs` — a doc comment, the filter
predicate, and seven fixtures — **so the control SELF-MATCHES against that crate's own test
output.** ⭐ **Correct for a gate log, unsound for a cargo-test log. A needle's validity is
scoped to the artifact it was designed against.**

⭐ **THREE PANES RETRACTED THIS INDEPENDENTLY, EACH MEASURING RATHER THAN ACCEPTING THE
REFUTATION ON NARRATIVE** — including the one being corrected, which noted that accepting a
peer's prose was the exact defect it had been corrected for an hour earlier. **Taking a
correction on trust would have repeated the error inside the apology for it.**

## ⭐⭐ ONE PENDING RUN CLEARS **FOUR BEADS' CI HALVES AT ONCE** — measure the REPAIR, not only your own commit

**Three panes measured in-flight containment of one commit. A fourth measured the REPAIR that
ended the build blindness:**
```
12 of 12 in-flight runs CONTAIN 454328b        --limit 12, 1.94s
```
⛔ **`34577201755` scored ZERO on rung 3 because `gate-runner` did not compile. EVERY run now
pending carries the repair.** ⭐ **So the next completed run is simultaneously the first
verdict for the ledger bead, the first for the host-precondition bead (whose guards were in
the blinded run), and the first that can answer two other crates' legs.**

⭐⭐ **AND THE FALSIFIER IS PRE-STATED: if a run containing `454328b` STILL scores zero on
rung 3, that is a NEW defect, not the old one.** **Naming in advance which observation would
mean something different is what makes the pending verdict readable.**

## ⭐⭐ A NO-CLAIM THAT IS **SILENT WHERE A READER WILL FILL THE SILENCE** IS DOING HALF THE JOB

**A grader's close said *"`454328b` is PUSHED and has not appeared in a verdict-bearing run
that built"* — true, re-measured, still true, and it never said *"none is pending."*** ⛔ **It
refused the credit anyway:**

> *"I did not make the inference because I had no occasion to, not because I checked. My
> correct-by-omission close would still have left a reader inferring that nothing was coming."*

⭐ **Correctness by omission is not correctness by verification, and the reader cannot tell
them apart.** **It closed the gap at a cost of 1.94 seconds rather than resting on having
happened not to say the wrong thing.**

## ⭐ AND THE 9-vs-12 DELTA IS **TIME**, WHICH IS THE THIRD MOVING QUANTITY READ AS A DISAGREEMENT

**Not two instruments and not two populations.** Three runs started between the probes.
⭐ **Third instance tonight: the index that emptied between check and commit · `dirty=39` ·
the drained volume.** ⛔ ***Emptiness is an instant, not a state — and so is a run list.***
**Stamp the MINUTE on a run figure exactly as we now stamp the TREE on a test figure.**

## ⭐⭐⭐ AN `UNKNOWN` HAS **TWO DUTIES**: REPORTING IT AND RESOLVING IT

**The causal chain, traced end to end by the pane that started it:**
```
it reported two UNKNOWNs honestly and correctly refused to call them negatives
   -> left them standing for an hour, while the answer was FOUR SECONDS away
   -> a peer filled the gap with an inference
   -> a second pane awarded that inference `0 possible`
   -> a third adopted it
   -> one command falsified it
```
⛔ ***"I have been treating 'I said UNKNOWN' as discharging the duty. It discharges the
REPORTING duty and not the RESOLUTION duty."*** ⭐ **An unresolved CHEAP unknown is an open
invitation, and the honesty of the label does not close it.** **Reporting is necessary and not
sufficient.**

## ⭐⭐ HAND OFF A BLOCKED BEAD WITH THE RUNS THAT WILL ANSWER IT, BY NAME

**Rather than hold a blocked bead, it named the exact runs that can resolve it and the check
to apply, in order:**
```
1. grep -c 'GATE_RUNNER ' <log>       nonzero, or the gate produced no verdict
2. grep -c 'could not compile'        zero, or it is absent-by-build
3. GATE_RUNNER_FAILURE_CAUSE | grep -cE ':[0-9]+:[0-9]+:$'
     == total -> STILL header-only, the message fix did not land
     <  total -> messages are landing, the item is SATISFIED
```
⛔ **Three rungs, each of which can produce a false read on its own.** ⭐ **And rung 3 is ONE
CHARACTER: does `detail=` END at `:col:`?** ⛔ **NEVER use length — a header alone is 104-155
bytes, which is exactly why *"lengths 111-162, so it carries real panic text"* passed under
the live defect.**

⭐ **AND NEITHER IN-FLIGHT COUNT SHOULD BE PUBLISHED WITHOUT ITS MINUTE.** `9 at 08:2x` and
`12` minutes later are both correct samples; **a reader taking either as a STATE rather than a
SAMPLE repeats the entire class.**

## ⛔⛔ A **DISPLAY BOUND** REACHED A SENTENCE — `| head -4` published as a set

**Seventh member of the capture-wide family, and the block above inherited it: the answering
runs were reported as FOUR. The real figure is TWELVE OF TWELVE.**
```
the pipeline ended in  | head -4
the four named are real and contain the repair -- and so do the other eight
```
⛔ **A reader taking that list as exhaustive concludes eight pending runs CANNOT answer the
bead, when every one of them can.** ⭐ ***A `head` is a DISPLAY bound, not a MEASUREMENT
bound, and it must never reach a sentence.***

⭐ **Same tell and same remedy as a `{0,400}` grep bound, a `cut -c1-110`, a `cut -c1-600` and
a `| wc -l`: CAPTURE WIDE, FILTER AFTER.** ⛔ **Two of the seven now carry one author's name —
the first destroyed a `.code()` site with a zero-width window, this one destroyed eight rows
with a `head`.**

⭐⭐ **AND THE CORRECTED HEADLINE IS BIGGER THAN THE CORRECTION: ALL TWELVE pending runs carry
the repair, so the build blindness ends with the FIRST ONE TO COMPLETE** — and that single run
is simultaneously the first verdict for four separate beads. **The pre-registered
discriminator stands: if a run containing the repair STILL scores zero on rung 3, that is a
NEW defect and not the old one.**

## ⭐⭐⭐ THE `0 possible` LABEL, **EARNED** — take the extremum on the CORRECT SIDE of the population

**The retracted inference and the licensed one, side by side, because the difference is the
whole lesson:**
```
RETRACTED  "the newest VERDICT-BEARING head is an ancestor of my commit,
            so no in-flight run can contain it"
            -> substitutes a fact about COMPLETED runs for a claim about PENDING ones

EARNED     454328b committed            02:07:02
           OLDEST in-flight head        02:12:47   <- FIVE MINUTES LATER
            -> every pending run STARTED AFTER the repair, so none COULD lack it
```
⭐⭐ **Same claim SHAPE, and this one is licensed: the bound comes from a commit timestamp
against the OLDEST PENDING head — the extremum on the correct side of the population — not
from a fact about a different population entirely.** ⛔ **And it was spot-checked on the six
oldest heads rather than trusted from the loop.**

**`0 possible` is not forbidden. It requires quantifying over the set you actually mean, and
bounding it at the extremum that can falsify you.**

## ⛔⛔ MY OWN PIPELINE READ AN **ERROR** AS A NEGATIVE RESULT — this turn, while committing the rule

```
gh run list --limit 12 | jq '[…verdict-bearing…][0]'   -> Error: cannot use null as iterable
my script then ran:  merge-base --is-ancestor 454328b ""   -> nonzero
my script then printed:  "predates the repair"             <- FROM AN EMPTY SHA
```
⛔ **All twelve rows were `in_progress`, so the selector matched nothing — and my `&&`/`||`
chain converted a JQ ERROR into a confident negative about CI.** ⭐ **The denied-probe rule,
in my own pipeline, in the same turn I committed a block about display bounds.**

**A shell `||` branch cannot distinguish FALSE from FAILED.** ⛔ **Guard the sha for
emptiness before comparing, or the error path prints a verdict.** **Eighth member of the
family and the first where the instrument did not clip data — it manufactured an answer out of
an empty string.**

## ⭐⭐⭐ THE FEATURE'S OWN COMMIT PREVENTED THE BUILD THAT WOULD HAVE MEASURED IT

**Two predicates that looked independent collapsed into one, measured at source rather than
assumed:**
```
git log -S'let mut pending' -- crates/gate-runner/src/main.rs   ->  9176b51
```
⛔ **`9176b51` IS the message fix AND the commit whose unpaired consumer took the gate down.**
⭐ **So "contains the message fix" and "contains the repair" are NOT two conditions —
`454328b` DESCENDS FROM `9176b51`, so the repair STRICTLY IMPLIES the fix.** **One nested
predicate, not two intersecting ones, and every pending run satisfies both because satisfying
one is satisfying both.**

⭐⭐ **AND THE CONSEQUENCE IS THE TRAP'S TRUE SHAPE: the message fix has never run in CI NOT
because it landed late, but BECAUSE SHIPPING IT BROKE THE GATE THAT WOULD HAVE MEASURED IT.**
⛔ ***"`34577201755` contains the fix for the very defect whose absence its silence appears to
demonstrate."*** **`59 cause lines → 0` reads as a regression; the truth is that the feature's
own commit prevented the build.**

⭐ **AND TWO ERRORS SAT IN ONE LINE — a sample bound (`head -4`) AND an implied independence
between the two predicates.** **Only the first belonged to its author; the second was found by
a reader chasing the discrepancy the first had created.**

## ⭐⭐ VERIFY A RUNG'S **PRECONDITION AT AN IN-FLIGHT HEAD** — do not wait to learn it from the log

```
at an unfinished head:  LedgerRead  main.rs=2  lib.rs=2   ·  EXIT_LEDGER_UNREADABLE 1/1
```
⭐ **The outward pair check works on a head that has not finished building, so the pair is
provably COHERENT at the head under test — which is exactly what rung 3 depends on.** ⛔ **Two
commands on an unfinished head PREDICT a rung that otherwise has to be discovered from the log
afterward**, and they make the pre-registered falsifier precise: **a zero on rung 3 at a head
whose pair is coherent is a NEW defect, not the old one.**

## ⛔⛔ COMMIT **TIMESTAMP** IS NOT **ANCESTRY** — the third `0 possible`, and its argument is a correlate too

**The block above credits an earned `0 possible`. Its JUSTIFICATION was still a correlate, and
a peer caught it within minutes:**
```
argued:   fix committed 02:07:02 · oldest in-flight head 02:12:47 -> none could lack it
measured: tonight's six commits, pairwise SIBLINGS = 0. A fully LINEAR chain on main.
          commit-date order == ancestor order, EXACTLY.
```
⛔ **A newer commit does not CONTAIN an older one — it contains it only if it DESCENDS from
it.** **Two commits on divergent branches are each "newer" than the other's predecessor and
contain neither.** ⛔ **And committer dates are AUTHOR-CONTROLLED VALUES, not observations —
this fleet already lost an hour to a timestamp read across an undeclared timezone.**

⭐⭐ **THE CONCLUSION HOLDS, CONTINGENTLY: every head is on a single linear `main`, which makes
date order and ancestor order coincide.** ⛔ **The moment a run fires on a head that does not
descend from the fix, a later timestamp proves nothing.** ⭐ **The SIX-HEAD SPOT-CHECK is what
carries the claim; the timestamp reasoning does not — the same split as every other retraction
tonight: the measured half stands, the inference bolted on does not.**

⭐ **AND THE GENERALISATION: CONTAINMENT IS MONOTONE ALONG THE ANCESTOR ORDER, NEVER ALONG
COMMIT AGE.** On a linear `main` they coincide, which is why one commit reaching 12/12 while a
later one reaches 7/12 is **a partial order, not a contradiction.** ⛔ **A containment figure
moves on TWO AXES — stamp it `(commit, minute)`, not just the tree.**

## ⛔ A THIRD TIMEOUT, AND THE BUDGET DIAGNOSIS WAS ITSELF INCOMPLETE

**`--limit 12` timed out too.** ⭐ **So the cost was never only the row count — it is the
PER-ROW `git merge-base` SUBPROCESS LOOP.** ⛔ **A diagnosis that fixes one term of a cost and
declares the problem understood is the same shape as a reconciliation that explains one term
of a gap.**

⭐ **And the `UNKNOWN` was closed by three peers' independent measurements rather than by its
owner — exactly what the corollary predicts when a cheap unknown is left standing. It was left
standing twice, and its owner says so.**

## ⭐⭐⭐ A **VACUOUS CONJUNCTION** — "BOTH" manufacturing a filter that removes nothing

**Written independently by TWO panes about the SAME commit pair within an hour:**
```
"the FIRST runs in which the message fix AND the repair are BOTH present"
"ALL TWELVE ALSO CONTAIN 9176b51 -- alongside 454328b"

git merge-base --is-ancestor 9176b51 454328b -> YES
therefore contains(repair) IMPLIES contains(msgfix)
rows the conjunction could ever exclude: ZERO, BY CONSTRUCTION
```
⛔ **`BOTH` and `alongside` each smuggled in an independence that does not exist.** ⭐ **The
sentences reduce to "runs containing the repair" — and a third pane spent real effort chasing
the discrepancy the implied independence created.**

⭐⭐ **AND THE ARTEFACT IS THE SHARPEST OF THE SESSION: the commit that NAMES this defect sits
INSIDE the range one of them printed —** `e35edb7 docs(agents): two instruments that agree AND
share a failure mode produce no information`. ⛔ **Two containment checks on NESTED commits are
one instrument run twice: the degenerate case of that rule, committed while quoting it.**

⭐ **AND A CORRECTION THAT UNDER-CHARGES YOU IS STILL YOURS TO FINISH.** One author was
credited with a display bound only; **its single line carried TWO errors — the `head -4` AND
the vacuous conjunction — and it refused the generous split rather than banking it.**

⭐ **BEFORE WRITING `A AND B`, ASK WHETHER `A` IMPLIES `B`.** **A conjunction over a nested
pair is not a stricter filter; it is the same filter wearing a stronger word.**

## ✅ A PRE-REGISTERED DISCRIMINATOR THAT FIRED — and the corroborator I refused to promote

**The one outstanding claim of the session closed 2026-09-11, on run `34577876082` / `2deb06cd`,
against a discriminator written down BEFORE the run existed:**

```
header-only (detail ends ':NN:NN:')     0 of 55      <- was 59 of 59
rung 3 'GATE_RUNNER ' liveness          2            <- the exact integer predicted in writing
rung 3 'could not compile'              0
```

**Two things make this worth a block rather than a line.**

**1. THE CORROBORATOR AGREED AND I DID NOT PROMOTE IT.** `detail` length came back
`min=94 max=6351`, which "proves" the same thing — and length was **retracted as a primary an
hour earlier**, because a header is *long*, so "not short" is satisfied by exactly the defect it
was written to exclude. **A retracted instrument that happens to agree with the good one is still
retracted.** The temptation is real and one-directional: a discarded measure never asks to be
re-promoted when it *disagrees*, only when it agrees, so promoting-on-agreement is a ratchet that
only ever loosens. **State which instrument is the verdict and which is corroboration, and keep
the order you committed to before you saw the data.**

**2. THE PREDICTED INTEGER WAS EXACT, AND THAT IS THE CHEAPEST FORM OF THE WHOLE DISCIPLINE.**
`2` was written down as the healthy score — one emission plus one ci-citation quoting it
verbatim — before any run carried the repair. An exact pre-registered integer cannot be
satisfied by a coincidence the way "nonzero" can, and it costs one sentence at prediction time
against a whole retraction cycle at reading time. **Every figure this session got wrong was
read first and explained afterwards; the one that was stated first came back exact.**

 **NO-CLAIM, and it is the load-bearing half:** this proves the EMITTER carries messages at
55/55 in one verdict-bearing run. It proves **nothing** about whether any of the four failing
crates is fixed, and it does not retract `GATE_RUNNER_FAILING count=16` three runs ago —
**different populations at different heads, which is a partial order and not a contradiction.**

## ⛔⛔ RETRACTED, BY ITS REPORTER, WITHIN THE HOUR — "SEVEN LEGS, ONE DEFECT" WAS A TRACE LINE

**This block published a rule on a specimen that does not exist, and the specimen was the
load-bearing half. DO NOT CITE THE RETRACTED FORM.** It read: *seven legs emit one byte-identical
cause, so one behaviour closes seven at once; only the message collapses the set.*

**The shared string is `empty_staged: CLEAN staged_files=1 deletions=0 merge=None`, and it is a
PROGRESS TRACE, not a verdict:**

```
pre-commit-gate.rs:180   eprintln!("empty_staged: CLEAN staged_files={} …")
  UNCONDITIONAL on any invocation with staged files — emitted BEFORE the refusal vector,
  before commit_ratchets::run, before validate_project_agent, before validate_staged_rust_modes.
  Means "there are staged files, proceeding."   10 of 55 cause lines carry it.
```

⛔ **SO IT IDENTIFIES AN INVOCATION SHAPE, NOT A DEFECT — and this is `sc0h5`'s own rule
(a needle firing on every member of a class cannot discriminate within it) violated in the
comment announcing the result.**

⭐ **THE REUSABLE PART IS WHAT FELT LIKE CORROBORATION.** The reporter found a **single producer**
for the shared string and read that as confirmation of one root cause. **It is the evidence
against:** one **unconditional** producer is precisely what a non-discriminating string looks
like. **"Few producers" supports a common cause only when the producer is CONDITIONAL on the
property you are attributing.** An unconditional emitter with one call site is maximally shared
and maximally uninformative — the same shape as a source containing the whole population.

**What survives, narrower and still worth having: a leg count CAN over-report, and only the
message could tell you. Nothing here demonstrates that it DID.** No leg in this run has been
shown to share a defect with another.

⛔⛔ **AND THE REASON A RETRACTION HAD TO REACH THE COMMITTED TEXT RATHER THAN A BROADCAST:
A RULE PUBLISHED WITH A FALSE WORKED EXAMPLE GETS APPLIED BY PATTERN-MATCHING ON THE EXAMPLE.**
The prose said *"a leg count can over-report"*; the example taught **"collapse legs that share a
substring."** A reader takes the operation, not the caveat. **So the retracted form did not merely
state something false — it installed the exact `sc0h5` failure it was written to retire, as
doctrine, under the name of the agent who found that failure.**

⭐ **AND RULE 4 RUNS ONE LEVEL UP FROM WHERE IT WAS WRITTEN.** It says a *surviving* conclusion
shields a rotten argument. **This one did not survive — and still reached doctrine**, because the
reporter went looking for CORROBORATION of a conclusion it liked instead of attacking it, and the
argument looks strongest at exactly the moment you stop testing it. **The hazard is not only true
conclusions with bad bases; it is PLAUSIBLE conclusions corroborated rather than refuted.**
`/brennerbot`'s refuters-over-supporters, measured as a live cost inside one hour.

## ⭐⭐ AN `assert_eq!` PAYLOAD IS **THREE** LINES — the capture fix is real and INCOMPLETE

**Found by chasing the retraction above, which is the second time tonight that pursuing one's own
error produced the better finding:**

```
details announcing `assertion 'left == right' failed`      10 of 55
details containing "left:"                                  0
details containing "right:"                                 0
"left:" / "right:" anywhere in the 493,235-byte raw log     0 / 0    <- positive control
```

**Rust prints an `assert_eq!` MESSAGE on the header line and the OPERANDS on the two lines
beneath.** The emitter takes *header + the next non-empty line*, so it captures the human label
and **drops observed-vs-expected**. For those 10, the deciding values are unreadable by anyone.

⛔ **AND `assert_eq!` IS EXACTLY THE CLASS WHERE THE MESSAGE ALONE IS LEAST INFORMATIVE** — the
label says what was compared, the operands say what went wrong. So the fix moved 59 legs from
*location-only* to *message*, and the class that most needed the payload is the one still short
of it. **A capture repair sized by "how many lines now carry a message" cannot see this;
it needs a per-ASSERTION-KIND census.**

## ⭐⭐ WHEN AN INSTRUMENT DEFECT IS FOUND, RE-SCOPE EVERY LIVE CLAIM **BY THE FIELD IT USED**

**Volunteered unprompted by a grader whose ten beads were already closed, minutes after a
different pane found the `assert_eq` operand gap:**

> *"All three readings used leg NAMES from the crate-level failing lists, never the CAUSE TEXT —
> so the emitter's newly-found operand gap does not touch them."*

**Nobody asked, and that is the point.** A freshly-discovered instrument defect does not announce
which conclusions it poisons. The reflex is to defend the conclusion (*"my result still holds"*);
the correct move is to **name the FIELD each claim consumed and check whether the defect lives in
that field.** Here three CI readings drew on `GATE_RUNNER_FAILING names=…` while the defect lives
in `GATE_RUNNER_FAILURE_CAUSE detail=…` — **disjoint, so the immunity is STRUCTURAL rather than
lucky, and checkable by a reader who trusts nothing.**

⛔ **A BARE "MY RESULT STILL HOLDS" IS UNFALSIFIABLE AND THEREFORE WORTHLESS.** *"I used field X,
the defect is in field Y"* is refutable in one command. **State the field, not the confidence.**

 **The obligation runs to whoever FOUND the defect too:** a defect report must name the field it
corrupts, so downstream holders can run this audit without re-deriving the mechanism.

## ⛔⛔ `0 passed` IS THE **DENOMINATOR** CHECK — and it is the hole in the two-proof-line rule

**Measured 2026-09-11 by a grader re-executing someone else's fix, on its first attempt:**

```
cargo test -p gate-runner --lib    test result: ok. 0 passed; 0 failed    exit=0
cargo test -p gate-runner --bins   test result: ok. 26 passed; 0 failed   exit=0
gate-runner's tests live in src/main.rs (15) and src/lib.rs (9) -- the BIN carries them
```

⛔ **BOTH PROOF LINES WERE PRESENT AND BOTH WERE VACUOUS.** This file's rule — *paste
`Remote command finished: exit=<N>` AND `test result:`* — **checks that the lines EXIST and never
that the denominator is nonzero.** A green `test result: ok. 0 passed` satisfies it completely
while executing nothing. Had the grader stopped there it would have filed *"confirmed green"*
from a run of zero tests, as a non-author, on the fix for a defect it had itself found.

**THE RULE IS NOW THREE PARTS: the exit code, the `test result:` line, AND A NONZERO `passed`
COUNT YOU EXPECTED IN ADVANCE.**

⭐ **AND THE TWO FAILURE MODES ARE MIRRORS, HIT BY TWO PANES INSIDE TEN MINUTES.** One:
`exit=101` with **no** `test result:` — absent-by-build, an `E0282` from the fix's own author.
Two: `exit=0` **with** a `test result:` — absent-by-selection. **A missing line that means
nothing ran, and a present line that also means nothing ran.** Only the count separates them.

## ⭐ A NEGATIVE NEEDS A **BOUNDED DENOMINATOR**, OR IT IS AN UNBOUNDED GREP

**A bead whose premise was *"these four legs fail on every Linux host"*, discharged like this:**

```
the failing list: 398 bytes · ZERO ellipsis · FIVE named legs · none of them the four
```

**Not *"I grepped and found nothing"* but *"I read the WHOLE list, it is complete, and the four
are not in it."*** A zero from a grep is consistent with a truncated source, a clipped field, a
wrong needle **and** a genuine absence; an enumerated list with no elision distinguishes them.

 **THE ELLIPSIS CHECK IS THE CHEAP HALF AND THE ONE EVERYONE SKIPS.** This repo has paid for it
twice — a `{0,400}` clip that manufactured a six-leg figure from a 2951-byte line, and a
`--limit` that turned a population into a sample. **Before citing an absence from a list: byte
length, elision marker, and a count that adds up.**

### ⛔ "YOUR" IN A `to: all` BROADCAST HAS **NO ADDRESSEE** — and it misattributes by default

**Measured immediately: the rule above landed in the FILE correctly unattributed ("a grader"),
and my BROADCAST of it opened `YOUR --lib NEAR-MISS`, sent `to: "all"`.** A pane that did not
run `--lib` read it as aimed at itself and **declined the credit in writing**, having checked its
own record first. **The commit was clean and the announcement was not.**

**A broadcast has no second person.** In a DM `you` resolves; fanned out to five panes it
resolves five ways, and every recipient who *could* plausibly be the referent will assume they
are. **NAME THE PANE, OR WRITE IT IN THE THIRD PERSON.** This costs four characters and is the
difference between a record and a rumour.

⭐ **THE DECLINE IS THE REUSABLE PART, and its author named why it matters:** it had been
corrected four hours earlier for labelling eight dirty files as a named peer's work when they
were unowned — *"the measurement was right and the attribution was not."* **Accepting a credit
you did not earn is that defect with the sign flipped, and it is STRICTLY MORE CORROSIVE,
because nobody is motivated to check a flattering attribution.** A wrong blame gets contested by
the blamed; a wrong credit is contested by nobody.

 **AND FIRST OBSERVER ≠ LATEST OBSERVER.** The `--lib` → `0 passed` shape was first reported
hours earlier by a third pane grading a different bead, with the correct mechanism (*the tests
live in a BIN*). Tonight's pane re-hit it independently. **Both are real; "a grader" in the
committed text is accurate for both and is why the file survived the broadcast's error.**

### ⭐⭐ A PEER RE-RAN MY NEGATIVE AT A **STRICTER SCOPE THAN I CLAIMED** — and it held

**My missing-emitter limit rested on `grep -rln 'Remote command finished' crates/*/src/*.rs` — a
ONE-LEVEL glob that cannot see `src/bin/`, `src/hook/`, or any nested module.** That is the
under-scoped-needle family this repo keeps retiring, inside the sentence where I was being
careful about scope. A peer re-ran it recursively:

```
my glob     crates/*/src/*.rs           -> 0
recursive   --include='*.rs' crates/    -> 0
```

**Both zero, so the claim survives at a scope I did not earn.** ⛔ **That is luck, not rigour —
a one-level glob returning zero is `UNKNOWN` about the nested population, and it would have read
identically had an emitter sat in `src/bin/`.** **When your evidence is an ABSENCE, the glob is
the claim.** Recursive by default; state the scope you actually searched.

## ⭐⭐ OUR GATE ALREADY HAD THE DENOMINATOR CHECK OUR **PROTOCOL** LACKED

**Found one tick after landing the three-part proof rule, by asking whether our own instrument
makes the mistake the humans were making.** `gate-runner` types it:

```rust
pub enum NoTestsDisposition {
    DeclaredException { reason: &'static str },   // listed in NO_TESTS_ALLOWANCE, with a reason
    Undeclared,                                   // "An ERROR — a crate with no tests at all
}                                                 //  cannot gate anything."
```

and the summary line carries `no_tests=` as its own column beside `pass` / `fail` / `unmeasurable`
/ `short`. **A crate that runs zero tests is fail-closed with a named allowance, exactly as this
file demands of every gate — while the human callback contract accepted `test result: ok.
0 passed` as proof for weeks.**

⛔ **THE ASYMMETRY IS THE LESSON: the CODE path was held to a standard the PROSE path was not.**
A gate gets reviewed, mutation-tested and argued over; a protocol sentence in a markdown file
gets copied. **When you find a hole in a procedure, check whether the equivalent code already
solved it — the fix may be a transcription rather than a design.** And the converse is the more
common direction here: `93lo` exists because the protocol has no emitter at all.

## ⛔ GIT AUTHORSHIP CANNOT ESTABLISH GRADING INDEPENDENCE — **EVERY PANE COMMITS AS `Josh`**

```
git log -1 --format='author=%an committer=%cn' c79524e   ->   author=Josh committer=Josh
```

**Every commit in this repository, from every pane, carries the same author and committer.** So
the grading gate's *"a bead is closed by an agent who did NOT implement it"* **cannot be checked
against git**, and an agent trying to prove its own non-authorship from commit metadata will find
the field uniform and uninformative.

**This is the `pane=` rule reaching the VCS layer.** This file already records that an agent NAME
is not an identity — `WildStone` binds three panes — and that `pane=` is the only unique field in
an assignee string. **Git is worse: it has no pane field at all.** The independence evidence is
therefore the **tracker's `assignee` history plus the bead's comment trail**, never `git log`.

 **OPERATIONAL CONSEQUENCE, and it is why this is not a curiosity:** when you cannot establish
non-authorship, **do not close — record the measurement as a comment and route the close.** That
is strictly cheaper than a contested close, and it is what an honest `UNKNOWN` looks like at the
identity layer.

## ⭐⭐ THE STRONGEST CONFIRMATION IS AN **INVERTED NEGATIVE CONTROL** WITH THE NEEDLE UNCHANGED

**Same grader, same session.** Its original finding was an absence: four message substrings
measured at **ZERO** across a 477,546-byte log. Against the repaired emitter, **the identical
needles** return `allowlist label 1 · live bead 4 · unstamped binaries 1 · ceiling 10`.

**That is worth more than a fresh positive measurement, and the reason is mechanical.** A new
probe confirming a fix leaves open that the probe is new — different pattern, different scope,
different bug. **An inverted control holds the instrument FIXED and moves only the subject**, so
the one thing a re-measurement usually cannot rule out is ruled out by construction. **The needle
that proved the absence proves the presence.**

 **This is the positive-control rule (`8i`) run BACKWARD IN TIME**: instead of proving your
instrument can return nonzero before believing a zero, you keep the zero-returning instrument and
show it now returns nonzero. **When you report an absence, WRITE THE NEEDLE DOWN VERBATIM** — it
is the asset that will confirm the fix later, and re-typing it from memory destroys the property.

## ⭐ **RIGHT CALL ON INSUFFICIENT GROUNDS** — vindication does not validate the reasoning

**The eighth leg was the one the grader had REFUSED to move, calling its own evidence weak.** It
is now vindicated by **three independent signals converging**: its cause is environmental
(`Committer identity unknown` — git config, not a gate verdict), it is the only row whose
LOCATION differed across lanes, and the only one whose assertion FORM differed (`CODE` vs
`SUCCESS`). **It had declined on the weakest of the three and had not measured the other two.**

**Say it in exactly that shape when it happens to you.** A correct decision reached on
insufficient grounds is a **near-miss**, not a win: the same reasoning applied to the next row
would have been wrong, and the only thing separating the two cases is evidence the agent did not
have at the time. **Recording it as vindication launders a coin-flip into a method.**

## ⛔⛔ THREE BASES FOR ONE TRUE SENTENCE — a surviving conclusion HIDES a rotten argument

**Self-reported by a grader that had to retract the same claim's SUPPORT three times while the
claim itself stayed true every time:**

```
basis 1   a SAMPLE presented as a bound            retracted
basis 2   a TIMESTAMP correlate recorded as structural   retracted
basis 3   a sample that was not even of the right population   retracted
conclusion  "the blindness ends when a run containing the repair completes"   TRUE throughout
```

**Basis 3 is the sharpest: it published *"12 of 12 in-flight runs contain the repair"* and the run
that actually delivered the result WAS NOT AMONG THE TWELVE** — an older head finished while the
newer twelve still ran. The census sampled the **newest** twelve and was described as enumerating
the **pending** population.

⛔ **THE MECHANISM, and it is the reason this needs a name: a conclusion that keeps surviving is
exactly the condition under which a bad basis never gets examined.** A false conclusion gets
refuted and drags its argument down with it. A true one shields every argument ever offered for
it — each new basis is *confirmed* by the outcome, so the error is invisible at the only checkpoint
anyone runs. **Truth is not error-correcting for reasoning; it is error-CONCEALING.**

**The remedy is to audit the BASIS on its own terms, separately from the claim.** Ask "is this
sample the population I named?" *before* asking "did the conclusion hold." And when you supply a
second basis for something you already believe, treat that as a **signal that the first was weak**,
not as reinforcement — **nobody reaches for a third argument for a claim whose first one worked.**

## ⛔ ABSENCE FROM A FAILING LIST MEANS **DID NOT FAIL**, NOT **RAN AND PASSED**

**A fifth verdict class, and the one that reads most like a win.** Four legs guarded by
`cfg`/precondition skips were absent from a 493 KB log, and the honest reading is that **CI is a
second confirmation of the SKIP path, not a first observation of the ASSERT path.** ubuntu-latest
has no Darwin toolchain, no wrapper on `PATH` and no registered-root mount, so the preconditions
those legs assert under **cannot hold there**.

**Distinguish three things a quiet leg can be, because they have three different remedies:**

```
PASSED      ran, asserted, green            -> the property is evidenced
SKIPPED     precondition false, never ran   -> the property is UNMEASURED, and always will be here
ABSENT      not compiled / not selected     -> you measured the harness, not the leg
```

 **A `NO-CLAIM` naming an unreachable lane is not discharged by a run on a lane where the code
cannot execute.** The grader who owned these refused to let its own good news discharge its own
limit — **which is the only reliable defence, because the discharge always looks like progress.**

## ⭐⭐⭐ THE CORRELATE IS ALWAYS **CHEAPER** — the bias has a SIGN

**A second pane checked its OWN four failures against the unifying law rather than admiring
it, and it is four for four:**
```
                         I TESTED                      THE PROPERTY WAS
phantom-holder           absent from the roster        not claimed by a live agent
location => cause        same file:line:col            same failure CAUSE
.code() decomposition    token line == panic line      token INSIDE the assert
the control it proposed  unconditional emission        DOWNSTREAM of the measurement
```
⛔ **The fourth was committed AFTER the pattern was visible and WHILE ARGUING ABOUT
CONTROLS** — *unconditional* is a property that correlates with liveness and is not liveness.

⭐⭐ **AND THE ECONOMIC OBSERVATION IS THE OPERATIONAL HALF: the correlate is ALWAYS the
cheaper computation.** `wc -l` beats reading a window · a roster lookup beats a claim check ·
a token grep beats parsing an assert · `--is-ancestor` beats fetching a log. ⛔ ***"Tonight's
instruments failed in the direction of CONVENIENCE every single time, and none failed in the
direction of CAUTION."*** **That is a bias with a SIGN, not random error — so a correlate-based
instrument is not merely weaker, it is BLIND PRECISELY WHERE IT IS NEEDED, which is why every
one of them came back clean.**

⭐ **THE CHECK THAT PAYS, written independently by three panes from three different beads:
NAME THE PROPERTY · ASK WHAT YOUR COMMAND ACTUALLY MEASURES · ASK WHERE THEY DIVERGE.**

⭐ **And one probe that could not be completed was closed by someone else's measurement rather
than by its author finishing it** — a timed-out sweep stayed `UNKNOWN` while a peer's ancestry
check answered the same question from the other side. **That is the right outcome and worth
more than the completion would have been.**

## ⛔⛔ MY LIVENESS NEEDLE SCORES **2** AND ONE HIT IS A **QUOTATION**

**Found by following the property/observable rule one step further than anyone asked:**
```
line 4598  GATE_RUNNER crates=89 pass=81 fail=4 …            <- THE EMISSION
line 4708  {"schema_version":"…gate-runner-ci-citation/v2",
            "run_id":"34574702390", … }                       <- A CITATION STEP
           whose payload EMBEDS that same line VERBATIM
```
⭐ **The embedded `run_id` was CHECKED rather than assumed — same run, so it is a
SELF-citation and the control is sound here.** ⛔ **But the class is real: a healthy run
scores 2 and ONE OF THEM IS A QUOTATION. If a citation step ever embedded a PRIOR run's line,
the needle would fire on a run whose own gate produced nothing** — **reading a stale CI row,
one layer in.**

⭐ ***`grep -c 'GATE_RUNNER '` IS A LIVENESS CHECK, NEVER A COUNT OF GATE INVOCATIONS.***
**When a needle can match quoted text, verify the quote's provenance before counting it as an
event.**

⛔ **AND THE AUTHOR OF THAT CAVEAT IS ALSO THE SPECIMEN FOR THE CONVENIENCE BIAS, BY ITS OWN
ACCOUNT: it asserted *"the census is emitted EARLY"* about code it had not opened — INSIDE a
message auditing someone else's instrument.** ⭐ ***"My claim cost nothing to check and I did
not check it, because asserting was cheaper than opening the file."*** **That is the sign of
the bias stated in its purest form: the cheap option is not a shortcut to the answer, it is a
substitute for it.**

## ⛔⛔ `--nocapture` CANNOT SHOW A **SPAWNED CHILD'S** STDERR — an absence that is not evidence

**A grader tried to upgrade a transitive verification into a direct observation, ran
`--nocapture` to see an emission with its own eyes, saw NOTHING — and correctly refused to
report that as an absence.**

⭐ **The legs spawn `gate-runner` as a CHILD and capture its stderr into a `String`, so the
PARENT harness never sees it. `--nocapture` controls the parent's own capture and is
STRUCTURALLY INCAPABLE of surfacing the child's stream.** ⛔ **A blank result from an
instrument that could never have shown the thing is `UNKNOWN`, not zero** — and the honest
outcome was *"I read the assertions at HEAD and reddened them with my arm; I am not claiming
to have eyeballed the line."*

⭐ **TRANSITIVE VERIFICATION, DECLARED AS TRANSITIVE, BEATS A DIRECT OBSERVATION THAT THE
INSTRUMENT CANNOT MAKE.** **Trying to do better and failing, then saying so, is the outcome —
not a weaker version of the outcome.**

## ⛔ AN ARM THAT DOES NOT COMPILE MEASURES NOTHING — the second plant wrong by construction

**`E0308`: the ledger is a `BTreeSet`, not a `Vec`.** ⭐ **The grader reported the failed arm
alongside the corrected run rather than only the run that worked** — *"second time tonight my
plant was wrong by construction; the first destroyed a DEGREE OF FREEDOM, this one destroyed
the BUILD."*

⛔ **The two failure modes rank differently and both belong in the report: a non-compiling arm
is LOUD and costs a cycle; an arm that compiles and cannot move the property is SILENT and
produces a clean, plausible, wrong result.** ⭐ **Publish the arms that failed, not only the
one that bit.**

⭐ **AND ONE GRADE TONIGHT WAS NOT WORKTREE-ONLY:** `crates/gate-runner` was fully clean at
HEAD, every tracked file byte-identical, nothing in flight — **so its verdict is a HEAD
verdict, and `454328b` at `323/0` across three files discharges no-weakening structurally.**
**Contrast a sibling grade where the module deleted six asserts and each had to be traced by
name: THERE WAS NOTHING HERE TO LAUNDER.**

## ⭐⭐⭐ THE UNIFYING LAW: EVERY INSTRUMENT FAILURE TONIGHT SUBSTITUTED A **CORRELATE** FOR THE PROPERTY

**One pane's three failures, laid side by side by its own author — and they are ONE pattern,
not three:**
```
vacuity check      tested EMPTINESS            when the defect was WRONG CONTENT
pair sweep v1      tested ABSENCE-ANYWHERE     when the defect was ABSENCE-IN-THE-DEFINER
sweep selection    tested FILE COUNT           when the risk was PATHSPEC-VS-PAIR
```
⭐⭐ **Each substituted an observable that CORRELATES with the property for the property
itself — and EACH CORRELATE WAS CHEAPER TO COMPUTE, which is exactly why it was reached
for.** ⛔ ***"The correlate always passes the case where it diverges from the property, and
that case is the defect."***

**The whole session reduces to this. Every entry above is an instance:** a length standing in
for content · a location standing in for a cause · a token spelling standing in for an
assertion shape · a file count standing in for a pathspec · a cardinality standing in for
completeness · `clean` standing in for `healthy` · ancestry standing in for measurement ·
**and a name standing in for a scope.**

⭐ **THE OPERATIONAL FORM: when you pick a check, write down the PROPERTY and the OBSERVABLE
as two separate sentences, then name the case where they diverge.** ⛔ **If you cannot name
that case, you have not yet found it — you have only failed to look.** **And validate on BOTH
controls before trusting a single row: the known-bad must fail, the known-good must pass.**

⭐ **The corrected selection criterion, stated as the property rather than the proxy:**
*commits whose changed-path set does not contain every file defining a symbol the change
references.* **Computable, and what should have been written.**

### ⛔ A CLAIM STATUS TRANSCRIBED INTO A DISPATCH IS A VALUE, AND VALUES GO STALE

**Five instances in one session, all the conductor's:** a withdrawn ownership ruling two agents
had already settled between themselves; a phantom slot 4 in a queue, holding a peer for work that
would never come; a contention table naming a plant restored fifteen minutes earlier; a GO
published on `746a535` landing a file it never touched; and `rluzf` broadcast as *"unclaimed"*
while `br show` read `in_progress · pane=%33`.

⭐ **THE LAST ONE WAS CAUGHT BY THE RECEIVER, NOT THE SENDER:** it ran `br show` before touching
a file and stopped, **avoiding a two-agent collision on an 81-file P0.** ⛔ **`br show` costs one
command. A dispatch naming a holder, a queue slot, a free file, or an unclaimed bead MUST
re-derive it at send time** — this file's own transcribed-value rule, aimed at the dispatcher.


**`poumg.5` is the specimen that proves the pair is needed:** `7b3f78d` added
`use text_structure::code_and_literals` and **never added the dep**, so it compiled at neither
its own tree nor HEAD — **while every local AND remote run was green off a peer's UNCOMMITTED
`Cargo.toml`.** `rch` syncs the tracked worktree, so **the dirty manifest travelled with the
build** and the remote lane could not expose it either.

⛔ **THE LIMIT, from its author, so nobody over-reads it: this proves a CRATE compiles at HEAD,
NOT THE WORKSPACE. It CANNOT catch the caller-committed-callee-untracked case, because the
untracked file is IN the worktree and therefore INSIDE the closed input set — the hash
comparison never fires.** That one genuinely needs a clone or CI.

**THE THREE QUESTIONS, AND ONLY THE THIRD REQUIRES CI:**
```
per-crate "does it compile at HEAD"   LOCALLY ANSWERABLE  -- the two steps above
cross-file tracking completeness      clone or CI only    -- `git archive HEAD | tar -tf -`
whole committed tree compiles         CI ONLY
```

**NO-CLAIM.** CI compiles HEAD; it does not prove HEAD is correct, and its verdict is still only
as good as the gate's own legs. This row says what the instrument uniquely measures, not that the
measurement is sufficient. **And it remains true that 51 of the last 100 runs failed with no run
id or sha cited anywhere in `.beads/issues.jsonl`** — reachable, produced, and still unread is a
separate open problem.

---

## ⛔ THE KERNEL-BYPASS GUARD PROTECTED A SPELLING NOBODY TYPES (measured 2026-09-11)

**`kernel_candidate` recognised `omp-orchestrator` — the crate/bin name — and NOT `ompo`, the
name the binary is INSTALLED and INVOKED under.** The launchd row runs
`/Users/josh/.local/bin/ompo supervise --repo …`, and the *"kernel must be the sole shell
command"* rule is reachable **only after `kernel_candidate` returns `Some`**:

```
echo setup; omp-orchestrator --once   ->  DENY   "must be the sole shell command"   (pinned)
echo setup; ompo supervise --once     ->  ALLOW  "no registered kernel bypass command detected"
                                                                              ^^^ THE HOLE
```

⭐ **THE PROTECTION APPLIED TO A SPELLING NOBODY TYPES WHILE THE ONE EVERYBODY TYPES FELL THROUGH
TO THE PERMISSIVE TAIL.** This is this crate's OWN recorded defect class one level out — a
classifier reaching a permissive fallthrough for an input it failed to RECOGNISE, exactly as
`classify_hook_liveness` once made `COVERED` its default branch.

**THE GENERAL RULE: A GUARD KEYED ON A NAME MUST BE KEYED ON THE NAME THE THING IS *INVOKED*
UNDER, NOT THE NAME IT IS *BUILT* UNDER.** Crate name, bin name, installed name and argv[0] are
four different strings, and a guard that enumerates the wrong one is not weak — **it is absent
for the only path that matters**, while reading as present in every test that uses the build-time
spelling.

**And the diagnosis discipline is the reusable half:** the fixer named the remedy CLASS before
editing — not class 1 (the production string was intact and three sibling commands reached it),
not class 3 (the test name promised distinct verdicts and **the BODY was checked, not the name**)
— **class 2, the classifier genuinely failed to accept a valid shape.** The new leg asserts
BEHAVIOUR — which verdict for which input — rather than swapping one substring pin for another.

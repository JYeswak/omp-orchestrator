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

**For a macOS binary, the target goes in `--config`, NEVER in `--target`:**

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo build --release -j 2 \
  --config 'build.target="aarch64-apple-darwin"' \
  --config 'target.aarch64-apple-darwin.linker="/usr/local/bin/zigcc-aarch64-darwin"' \
  -p <crate> --bin <bin>
```

**SINGLE quotes outside, DOUBLE inside.** Drop them and cargo refuses with *"string values must be
quoted."* **COPY IT, do not type it from memory** — Joshua did and it cost a build.

**`--target` sets `required_os=darwin` and collapses the admissible fleet 4 → 1. That is the `rc=103`
cause.** `--config build.target=` produces the **identical binary** with the whole fleet admissible.

**DO NOT PIN A WORKER.** No `RCH_WORKER=`. All four boxes are the same class running the same
toolchain for the same asupersync builds, so pinning buys nothing and refuses often: measured
2026-09-07, unpinned `rch exec` succeeded **12+ times** including the Mach-O cross-build, while
pinned attempts returned `RCH-I005 project_excluded`.

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
   the tree — and is never a restore unless you have separately established the file was clean.

   **AND `git diff --numstat` IS NOT AN ORACLE ON AN UNTRACKED FILE** — it is vacuously empty
   there, so a restore "proven" that way proves nothing. Run it only against a tracked path, and
   prefer `sha256` or `cmp`, which do not care about tracking state.

   **NO-CLAIM.** This makes a restore *correct*; it does not make a mutation *attributable*. A
   leg still has to show the RED was caused by the mutation and not by unrelated breakage — the
   discriminator is that the other legs stay GREEN, per rule 7.

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

**The live gap is a missing `[[bin]]` on `crates/finding`** — that is the fix, and it is S1-authorized
work, not a doctrine note.

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

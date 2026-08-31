# 00 — The brief

**This file exists because the requirements were living in chat.** That is the same failure mode
this project was stood down to fix: the stand-down reap found **seven real conditions living only in
pane scrollback**, which dies with the pane. A requirement that exists only in a conversation is a
requirement that will be silently dropped the moment the conversation is compacted.

So the brief is written down first, verbatim, before the sections that satisfy it. Every section
file in `docs/plan/` is answerable to this document. If a section does not trace to a requirement
here, it is scope creep. If a requirement here has no section, that is a gap and it is visible.

Written 2026-08-31 during the stand-down. Authoritative until superseded in place.

---

## 1. The requirements, in Josh's words

These are quoted verbatim from the session that commissioned this plan. They are not paraphrased,
because paraphrasing a requirement is how a requirement quietly changes.

> **R1 — Stop and plan.** "i think its time we step back a bit. we've been running and gunning for
> a bit without a really great defined bead dag. I think we need to step back and go into a full
> process to define from a - z the entire project management journey, the gates, the crates, the
> schema, the types, and the entire plan - validation, define what done looks like at each
> milestone, and we work from that."

> **R2 — Reap, don't refill.** "lets not dispatch workers after they go idle this next time. lets
> reap work. lets update docs and stuff. lets ensure our repo is set up properly with good
> filestystem, good doc management structure."

> **R3 — One document, investor-grade.** "I want a single plan doc that we beat up and grade and
> turn into a - here is what we're doing - full /skill:readme-writing companion that would pass our
> github repo publishability bar with a future-tense on what we're building. it has to be something
> that could pass an 'investor' test - we give it to them and they can beat up the plan, find any
> gaps, and pass or fail us with their multi-decades of experience buying and selling and growing
> companies."

> **R4 — Total coverage of the system.** "i want this plan to cover every crate - every schema input
> / output, every typed interface, how everything is interacting, full frankenmermaid diagrams,
> etc."

> **R5 — Every OMP surface.** "every omp surface"

> **R6 — The gating frameworks.** "the testing / validation / gating frameworks that we are
> applying"

> **R7 — Mine the mirror at every gap.** "use fh - mine the dicklesworthstone projects along the way
> - anywhere we find a gap - we should ask - what would jeffrey do in one of his projects"

> **R8 — Installability.** "we need to have installability, how other repos and machines are going
> to use this, full /skill:canonical-cli-scoping"

> **R9 — All the way to end users.** "all the way to end users (other projects / repos / machines)
> are using it to orchestrate their projects"

> **R10 — The frame, and the SOTA bar.** "think about this plan as - idea - what it is, why we're
> doing it, the binaries we are wrapping, what the stated intended purpose of each action is with
> negative patterns, the outline of the high level what we're mapping, provide intricate design
> specs for each on what advanced framework we are applying and why - every aspect of this needs to
> be on par or greater than SOTA - same as the binaries we are wrapping."

> **R11 — Write it down before dispatching.** "get all of this documented in the planning doc so its
> written down and not in chat before dispatching team."

**R11 is the rule that produced this file**, and it is retroactively the most important one: it is a
process requirement about how requirements themselves are handled. It is now doctrine for this repo.

---

## 2. What each requirement demands, made checkable

A requirement that cannot be checked is a wish. Each row states the observable that closes it.

| id | demand | closed by |
|---|---|---|
| R1 | A-to-Z journey: gates, crates, schema, types, validation, per-milestone done | §09 exists and every milestone carries an OBSERVABLE |
| R2 | Reap before refill; repo hygiene; doc structure | Stand-down broadcast sent; board reaped (28/25/19/2); `docs/` added additively |
| R3 | One document, investor-attackable, future-tense on what we build | `docs/PLAN.md` assembles; §09 carries the grading rubric that invites failure |
| R4 | Every crate, every schema I/O, every typed interface, interactions, diagrams | §03 has one row per workspace crate; §04 has ≥6 generated diagrams |
| R5 | Every OMP surface | §02 enumerates all 183 census rows by kind with names |
| R6 | The testing/validation/gating frameworks | §06 gives an intricate design spec per framework |
| R7 | Mirror prior art at every gap | §10 gives a search command + verbatim quote or explicit not-found per gap |
| R8 | Installability + canonical CLI scoping | §07 specifies doctor/health/repair + validate/audit/why |
| R9 | End users orchestrating their own projects | §08 gives personas, a zero-to-first-tick walkthrough, adapters, degradation |
| R10 | Idea → why → binaries → actions+negatives → map → design specs at SOTA | §01 and §05; SOTA bar is operationalised per wrapped binary |
| R11 | Requirements written down before dispatch | **this file** |

**NO-CLAIM:** this table records that a section is responsible for a requirement. It does not
establish that the section discharges it well. Grading the sections is a separate pass, and §09
carries the rubric for it.

---

## 3. The measured facts the plan is built on

Every number below carries the command that derives it. These were measured 2026-08-31 in this
session. Sections must use these figures and must not re-derive them differently without saying so.

### 3.1 The binaries we wrap — `<bin> --version`

| binary | version | path |
|---|---|---|
| `omp` | `omp/18.0.11` | `/Users/josh/.local/bin/omp` |
| `ntm` | `ntm version v1.30.0-1-gda270719` | `/Users/josh/.local/bin/ntm` |
| `br` | `br 0.4.1` | `/Users/josh/.local/bin/br` |
| `bv` | `bv v0.20.0` | `/opt/homebrew/bin/bv` |
| `git` | `git version 2.50.1 (Apple Git-155)` | `/usr/bin/git` |
| `cargo` | `cargo 1.100.0-nightly (e8cb624d5 2026-08-2…)` | `/Users/josh/.rch/shims/cargo` (a shim) |
| `fh` | `franken-harvest 0.1.0+tree.7b0fc50c3e5a29d…` | `/Users/josh/.local/bin/fh` |
| `jsm` | `jsm 0.1.4` | `/usr/local/bin/jsm` |
| `tmux` | **rejects `--version`** (`tmux: unknown option -- -`) | `/opt/homebrew/bin/tmux` |

`tmux` is the one binary that refuses `--version` — but **it is not versionless**: `tmux -V` returns
`tmux 3.6a` at exit 0. `IdeaSection` challenged the first draft of this sentence, which claimed tmux
had "no machine-readable version handshake," and was right to. The corrected finding is narrower and
considerably sharper, because there is **no single version flag that covers the set**:

| flag | answers | fails |
|---|---|---|
| `--version` | 8 of 9 | `tmux` |
| `-V` | 5 of 9 | `omp`, `bv`, `git`, and `tmux --version`'s counterpart cases |

Only `ntm`, `br`, `cargo`, `fh`, `jsm` answer **both**. So a uniform probe loop must try more than
one spelling or it will record a present binary as absent.

**The first draft of this paragraph was wrong, and the error was mine — not tmux's.** It claimed
`tmux --version` "prints a usage block **and exits 0**", making tmux the exemplar of an exit code
that lies. `PriorArtWriter` re-measured without a pipeline and refuted it:

```
tmux --version >/tmp/t.out 2>/tmp/t.err ; echo $?
  exit=1   stdout_bytes=0   stderr_bytes=158

tmux --version 2>&1 | head -1 ; echo $?
  exit=0                       <- head's status, not tmux's
  PIPESTATUS=(1 0)
```

**tmux is well-behaved.** It fails, says so on stderr, writes *nothing* to stdout, and returns 1 —
textbook. The "exits 0" came from my own probe, which read `$?` after a pipeline, where the status
belongs to the last command. **My measurement harness laundered the failure into a success and I
then attributed the defect to the subject.** The exit-code-that-lies example belongs to the harness
and to our `installer` binary, not to tmux.

The corrected hazard points the **opposite way**, and it is the live one. A probe that treats
non-zero as *absent* records tmux — present, working, `3.6a` on `-V` — as **MISSING**. That is a
false negative on the presence of the one binary through which we read pane truth. It is not
theoretical: `pi_agent_rust/src/doctor.rs:924` gates `check_tool` on `output.status.success()` and
only forgives a failure through `probe_failure_is_known_nonfatal` at `:1052`, which hard-codes
`if tool.ne("sh")` — so tmux falls straight through to "invocation failed."

**This is the third self-inflicted error in this brief caught by a subagent, and all three are one
class:** a figure nobody recomputed (`1 of 8`), a search nobody re-ran without its harness artifact
(the `--include=` false zero), and a measurement nobody re-took without its pipeline (this one). The
swarm is functioning as the adversarial reviewer this plan argues it needs — against the conductor.
*All three recorded rather than quietly patched, because the failures are more instructive than the
corrected numbers.*

### 3.2 The OMP surface census

Produced by the built scanner: `/Volumes/BuildShared/cargo-targets/debug/omp-inventory-map`
→ 544,697 bytes of JSON, **exit 2**, envelope
`{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"UNKNOWN","data":{…}}`.

- **184 nodes, 207 edges, 183 rows.**
- Counts: `cli_commands=39`, `type_roots=57`, `declarations=14`, `rpc_handlers=42`,
  `slash_commands=0`, `omp_methods=3`, `workspace_crates=26`.
- **The scanner reports its own hole.** Every count has an `expected_*` twin. Six of the seven
  match exactly. One does not: **`slash_commands=0` against `expected_slash_commands=136`.** That
  single mismatch is why the envelope carries `status: UNKNOWN` and exits 2 — the scanner knows it
  failed to enumerate a surface it expected to find, and refuses to report success. **136 slash
  commands are the largest unmapped region of the OMP surface**, and they were missing from the
  first draft of this brief; `SurfaceCensus` caught it by comparing the twins, which is exactly the
  challenge the broadcast asked for. The census is not complete until slash-command enumeration
  either succeeds or carries a named reason. *Recorded under R11 — it was not written down before.*
- Row kinds: cli_command 39 · type_root 57 · rpc_handler 42 · workspace_crate 26 · declaration 14 ·
  omp_method 3 · transport 1 · slash_command 1.
- Classification: **`CAPABILITY_NOT_USED` 157** · `SCRAPED_OR_OBSERVED_ALTERNATIVE` 18 ·
  **`MAPPED_BY_DIRECT_PROBE` 8**.
- Edge relations: `provides` 157 · `map-to-none` 25 · `path-depends-on` 18 · **`consumes` 7**.
- **All 7 `consumes` edges originate from one crate, `omp-inventory-map`**, each carrying the
  evidence string *"direct process probe produced this row"*. 25 of 26 crates consume zero OMP
  surface.

The previously-published figure **"81 JSON-RPC methods, 17 used" is RETIRED** — it was not
re-derivable. The measured surface is 39 CLI subcommands, 71 type-surface entries (57 dirs + 14
top-level `.d.ts`), one `--mode=rpc` transport, and **3** methods matching `omp/*`.

### 3.3 The vacuity finding — against ourselves

```
python3 -c "…Counter(json.dumps(r.get('must_be_true')) for r in rows)…"
  crate rows:     n=26   distinct must_be_true=1  distinct negative_evidence=1
  non-crate rows: n=157  distinct must_be_true=1  distinct negative_evidence=1
```

All 183 rows carry the four mandatory fields (`inputs`, `outputs`, `must_be_true`,
`negative_evidence`) with **zero missing** — and exactly **one distinct value** of `must_be_true`
and **one distinct** `negative_evidence` across the entire census.

**The four-field discipline this orchestrator demanded of every worker is satisfied syntactically
and vacuously.** The universal invariant is
`["The source probe is non-empty before a known verdict is emitted.","A versioned inventory envelope carries the probe state."]`
and the universal negative evidence is
`["No repository source grep was used; ownership is derived from metadata and direct probes."]`.
For crate rows, `inputs`/`outputs` describe **the scanner's provenance**, not the crate's contract.

This is the sharpest finding of the session and it indicts the conductor, not a worker. It is the
same anti-vacuity property we require of every gate, failing on our own inventory.

### 3.4 The crate dependency graph — all 18 `path-depends-on` edges

```
ack-spine                 -> finding
finding                   -> subprocess-contract
ack-stage                 -> receiver-receipt
ack-stage                 -> tick-monitor
receiver-receipt          -> tick-monitor
finding-dispatch          -> finding
finding-dispatch          -> omp-orchestrator
omp-orchestrator          -> ack-stage
omp-orchestrator          -> dispatch-claim-fence
omp-orchestrator          -> omp-rpc-session
omp-orchestrator          -> receiver-receipt
omp-orchestrator          -> subprocess-contract
kernel-only-operator-hook -> subprocess-contract
no-shell-gate             -> path-literal-guard
no-shell-gate             -> pre-delete-citation-check
no-shell-gate             -> state-wildcard-lint
no-shell-gate             -> undrained-pipe-lint
pane-dispatch-fence       -> subprocess-contract
```

`subprocess-contract` is the most-depended-on crate (4 dependents). That is the correct shape: it is
the asupersync process-group / drain-both-pipes boundary, so everything that spawns should route
through it.

### 3.5 The gate framework, measured

```
find crates -name '*.rs' -path '*/tests/*' | wc -l   ->  26 integration test files
grep -rc '#\[test\]' …                                -> 370 #[test] fns
```

| crate | tests | known_bad | known_good | mutation | anti_vacuity |
|---|---:|---:|---:|---:|---:|
| `no-shell-gate` | 34 | 4 | 3 | 2 | 6 |
| `omp-inventory-map` | 23 | 0 | 2 | 1 | 1 |
| `undrained-pipe-lint` | 10 | 1 | 1 | 1 | 3 |
| `commit-build-fence` | 10 | 0 | 1 | 0 | 0 |
| `state-wildcard-lint` | 9 | 1 | 1 | 1 | 0 |
| `kernel-bypass-gate` | 6 | 1 | 1 | 0 | 0 |
| `pre-delete-citation-check` | 6 | 1 | 1 | 0 | 0 |
| `path-literal-guard` | 3 | 1 | 0 | 0 | 2 |

**2 of 8 gates have all four legs** — `no-shell-gate` (4/3/2/6) and `undrained-pipe-lint` (1/1/1/3).
**4 of 8 have no mutation leg** — `commit-build-fence`, `kernel-bypass-gate`,
`pre-delete-citation-check`, `path-literal-guard`. 2 of 8 have no known-bad. 1 of 8 —
`path-literal-guard` — has **no known-good leg**, which makes it the highest-risk gate in the set:
an attack-only suite ships an over-strict gate, and an over-strict gate gets routed around, which is
a slower death than no gate at all.

> **The first draft of this headline said "1 of 8" and "5 of 8," and both were wrong against the
> table printed directly above them.** `GateFrameworks` recomputed from the table and caught it.
> This is the purest instance of the defect this whole document exists to prevent: **a transcribed
> headline that nobody recomputes**, sitting one line below the data that refutes it. Prose review
> does not catch it — only arithmetic does. It is the same family as the retired "81 JSON-RPC
> methods, 17 used" figure, except this one was self-inflicted **in the brief that forbids it**, by
> the conductor, in the act of writing the rule. Every section quoting "1 of 8" or "5 of 8" must be
> corrected at assembly.

**Measurement hazard — shell `grep -r --include=` returns empty instead of failing.** Also found by
`GateFrameworks`, which measured 0 files matching `#![forbid(unsafe_code)]` while the harness grep
returns **55**. Verified both directions: `grep -rl 'forbid(unsafe_code)' --include='*.rs' crates`
→ `0`, quoted or unquoted; the same search without `--include` works and reproduces this table
exactly. So the leg table above is sound, and any figure in any section derived with an
`--include=` shell grep is a **false zero**. A blocked tool that returns empty rather than erroring
is precisely the never-silent-fail violation we gate against — in our own measurement path.
*Recorded under R11.*

**An honest skip is not a useful test — found by `EndUserJourney`.** `composer-typed` ships a
differential oracle, and the discipline in it is genuinely first-rate: `tests/differential.rs` types
the absence of its comparison target as `OracleStatus::MissingScript(PathBuf)`, announces the skip
loudly with the exact resolved path, and its header names the precise failure it exists to avoid —
*"the report looked differential while nothing differential ran."* That is anti-vacuity implemented
correctly.

And the oracle it compares against **cannot exist in this repo**. Line 41 resolves it to
`../../bin/composer-typed.py`; `ls bin/` returns *No such file or directory*, and
`find . -name 'composer-typed.py'` returns nothing, because **the no-`.py` rule forbids it**. So the
test is green forever and can only ever skip. Our hardest rule deleted the oracle our differential
test needs.

This is not the vacuity defect — the skip is typed and loud, which is exactly right. It is a
different and less obvious failure: **a test that is permanently unable to run is indistinguishable
from a passing one in any aggregate count**, and it sits inside the 370 `#[test]` figure quoted
above. The fix is a policy decision the plan must make rather than dodge: either the oracle lives
outside the tracked tree as a release artifact, or the differential lane is retired with a named
reason, or the rule gets its first exemption. *Recorded under R11 — not previously written down.*

### 3.6 The addressability defect — how the sixth gate property was born

`omp-inventory-map --help` returns:

```json
{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"ERROR",
 "data":null,"error":"CONFIG_ERROR unknown argument --help"}
```

The gate is **built, correct, and undiscoverable**: 13 tests pass and
`crates/omp-inventory-map/src/types_inventory.rs:176-178` deliberately excludes `Observation` from
the allowance list so the collision demands convergence — but the running binary's 544 KB doctor
output contains **zero** occurrences of `Observation`, `CONVERGE`, or `Verdict`.

Not built-vs-wired. **Wired-but-unaddressable.** It adds a sixth required gate property:
**ADDRESSABLE** — one documented command runs it, and `--help` names that command.

### 3.7 Other binding facts

- **The one hard rule:** no `.sh`, no `.py`. A Rust gate walks `git ls-files` and fails the build on
  either extension. The exemption list is empty.
- **The async contract:** asupersync 0.4.9, pinned rev `fa3c01aec`. `&Cx` first; `cx.checkpoint()`
  in loops; region-owned tasks, no detached tasks; **kill the process GROUP, not the pid**; drain
  both pipes; **a timeout is not a verdict**.
- Measured conformance across 29 raw spawn sites: 4 crates use `subprocess-contract`; 12 of 14 async
  fns take `cx` first.
- **Unsafe — corrected by `CrateSpecs`.** The first draft said "16 of 22 forbid unsafe," which had
  both an unstated denominator and an unstated mechanism. Measured: **20 of 26** declare
  `unsafe_code = "forbid"` in the manifest; **25 of 26** carry `#![forbid(unsafe_code)]` as an inner
  attribute; **19 of 26** carry both; the **union is 26 of 26**. The finding survives correction and
  is worse than it looked: total coverage holds by **two independent habits with no single
  enforcement point**, so either habit lapsing on a new crate is invisible.
- **`omp-types` — corrected by `CrateSpecs`.** The first draft claimed it re-exports `AckKind`,
  `DeliveryClass`, `ObligationLedger`, `Budget`, and `Outcome`. **That is wrong.** Measured with
  `grep -c` against `crates/omp-types/src/lib.rs`: `ObligationLedger` occurs **zero** times, and
  `AckKind`/`DeliveryClass` occur only inside the doc comment that names them as blocked. What
  actually re-exports is the `Outcome` family, the `Budget` family, and `ObligationId` / `RegionId` /
  `TaskId` / `Time`.

  The reason is documented in `crates/omp-types/Cargo.toml:11-17`: `AckKind` and `DeliveryClass`
  live behind `#[cfg(feature = "messaging-fabric")]`, that feature transitively needs
  `#[cfg(any(test, feature = "test-internals"))]`, and upstream issue #46 correctly removed
  `test-internals` from the default set — so enabling it here would **reintroduce the exact
  production leak #46 closed**.

  This changes the plan, not just the sentence: **the half of the vocabulary that would collapse the
  three ack dialects is blocked at an upstream feature boundary, not merely unadopted.** Any
  migration schedule assuming `AckKind` is available today is wrong. The crate still has **zero
  dependents**.
- Type inventory — two scopes, published as an error bar rather than one figure. Excluding test
  modules and bin sources: **51 public enums, 79 structs** across 22 of 24 crates. Including them
  (`grep -rhoE` over all `*.rs`): **59 enums, 91 structs**. Both scopes agree exactly on the figures
  that matter: **4 colliding type names** (`Finding`, `LintReport`, `Observation`, `Violation`),
  **6 Verdict-shaped types with no shared trait** (`AckVerdict`, `FenceVerdict`, `FollowUpVerdict`,
  `ReceiptVerdict`, `SilenceVerdict`, `Verdict`), and **17 ack/receipt types in 3 incompatible
  dialects**.
- `fh` MCP is failing closed with a typed `SERVE_INPUT_STALE` (mirror HEAD moved `5dec4212…` →
  `ecdea397…`). Direct grep of the mirror at `/Volumes/ZestData/dicklesworthstone-mirror` still
  works. **Failing closed with a remediation hint is the model**, not a defect.
- **Mirror size — corrected. The first draft said "216 repos" and that figure is re-derivable from
  nothing.** `PriorArtWriter` and `Installability` both flagged it independently. Four defensible
  counts, all measured:

  | count | command | meaning |
  |---:|---|---|
  | 218 | `ls $M \| wc -l` | visible entries, including files |
  | 217 | `find $M -maxdepth 1 -type d \| tail -n +2 \| wc -l` | directories |
  | **210** | `find $M -maxdepth 2 -name .git \| wc -l` | **actual git work-trees** |
  | 1 | `ls $M \| grep -c corrupt` | `.corrupt-`suffixed copies |

  **Any "N repos" claim must use 210** — it is the only count that counts repositories rather than
  filesystem entries. 216 matched none of the four, which makes it the third unstated-denominator
  defect in this brief after the retired "81 JSON-RPC / 17 used" figure and the `2/2`→`2/0` drift
  ratio. A denominator nobody can reproduce is not a measurement.
- Board at stand-down: **28 closed, 25 in_progress, 19 open, 2 blocked** (75 total).

---

## 4. The four-layer reality — what works and what does not

This is the spine of the whole plan. Exactly one row works today.

| layer | mechanism | measured state |
|---|---|---|
| observe | `tick-monitor` | **WORKS, WITH A MEASURED ASYMMETRY DEFECT** — see below |
| actionable | `idle_panes` | **BROKEN** — discards `NewlyIdle`; `free_capacity` derives from the same `is_dispatchable` filter, which requires *Confirmed* Idle, so a pane at `t=0` is excluded from **both** lists |
| consume | `decide()` | **FENCED** — 162 refused ticks over 4.2 hours, `DISPATCH_RETRY_BLOCKED` |
| actuate | dispatch | **DOES NOT EXIST** — a human types into panes |
| complete | worker says done | **DOES NOT EXIST** — every completion this session was found by a human looking |

**The observe row was downgraded by `ActionsNegative`, and the defect is in the one layer this brief
called working.** The two-capture rule has a genuine asymmetry: a **changed** content hash proves
motion at *any* interval, while an **unchanged** hash proves nothing below the 75-second floor. The
floor should therefore gate only the *idle* direction. It gates both:

```rust
// crates/tick-monitor/src/lib.rs:541-545  — the early return
let gap = now.at.saturating_sub(prev.at);
if gap < MIN_GAP_SECS {
    return Liveness::Unproven { why: "gap_too_short" };
}
// …:552 — the hash comparison it precedes, and therefore prevents
if b > a || prev.hash != now.hash { Liveness::Live } else { Liveness::Frozen }
```

A sub-floor capture pair whose hash **changed** is discarded as `gap_too_short`, so positive
liveness evidence the system already holds is thrown away. This does not make `observe` broken — it
never reports motion as stillness, which is the dangerous direction — but it is **lossy in the safe
direction**, and it means the fleet waits 75 seconds to learn something it could know in 20.

Two things make this worth the space it takes. First, it was found by an agent **disagreeing with
its own spawn instructions**: the dispatch brief asserted the asymmetry as implemented behaviour,
`ActionsNegative` read the source, and wrote it up as an open defect instead of repeating it.
Second, it is the *only* layer this brief marked unqualified `WORKS`, and it did not survive first
contact with the source. **The four-layer table now has zero unqualified working rows.**
*Recorded under R11.*

---

## 5. Section map — who owns what

Ten section files, disjoint by construction so that parallel authorship cannot collide. The
assembled `docs/PLAN.md` is built from these; the sections are the source of truth.

| file | section | requirement served |
|---|---|---|
| `00-brief.md` | **this file** — requirements, measured facts, section map | R11 |
| `01-idea.md` | The idea; why; the binaries we wrap; the SOTA bar | R10, R3 |
| `02-surface-census.md` | Every OMP surface, enumerated; coverage; the vacuity finding | R5, R4 |
| `03-crates.md` | Every crate: contract, schema I/O, typed interfaces, deps | R4 |
| `04-diagrams.md` | FrankenMermaid, generated from measured edges | R4 |
| `05-actions.md` | Every action: purpose + negative pattern it must refuse | R10 |
| `06-gates.md` | Testing / validation / gating frameworks, design-spec depth | R6 |
| `07-installability.md` | Distribution, identity, canonical CLI contract | R8 |
| `08-end-users.md` | Foreign repos/machines orchestrating their own projects | R9 |
| `09-milestones.md` | Milestones, done-definitions, plan validation, grading rubric | R1, R3 |
| `10-prior-art.md` | Mirror mining: what would Jeffrey do, per gap | R7 |

---

## 6. The writing contract every section obeys

These rules exist because each one has already been violated in this repo and cost real time.

1. **Every number carries the command that derives it.** A bare number is a guess wearing a
   uniform.
2. **`MEASURED` and `PROJECTED` never share a sentence.** The most valuable review finds a
   `MEASURED` that is actually `PROJECTED`.
3. **Every load-bearing claim ends with a `NO-CLAIM:`** stating what it does not cover.
4. **No unstated denominators.** Two measured instances of this exact defect: the retired
   "81 JSON-RPC methods, 17 used" figure, and a drift ratio where excluding a foreign binary
   decremented the wrong variable and turned `2/2` into `2/0`.
5. **Gate claims are floor-raises, never guarantees.** A residual "guarantees" / "proves" /
   "makes impossible" in a gate header is itself a defect, because a reader stops looking.
6. **Diagrams are generated from measured data, never drawn.** A diagram whose edges cannot be
   traced to data is forbidden; a target-state diagram must be captioned `PROJECTED`.
7. **Where there is a gap, ask what Jeffrey would do** — and cite the mirror repo/file/line, or
   state plainly `searched <pattern>, no prior art found`. A not-found is a valid, valuable result.
8. **Write it to be failed.** State the strongest version of the objection an investor would raise,
   then answer it or concede it.

---

## 7. What the writing of this brief proved

This section is the most investor-relevant thing in the document, and it is not about the product.

Ten agents were given the brief in §3 as *settled knowledge* with one instruction: start from it as
learned, do not re-derive it, but **challenge it with evidence if you disagree**. They challenged it.
In one session they refuted **five measurements**, three of them the conductor's own, and every
single refutation came from an agent **re-deriving rather than reading**.

| # | claim | refuted by | mechanism of the error |
|---|---|---|---|
| 1 | "1 of 8 gates has all four legs / 5 of 8 lack mutation" | `GateFrameworks` | arithmetic never recomputed — the table one line above said 2 and 4 |
| 2 | "tmux fails while exiting 0" | `PriorArtWriter` | `$?` read after a pipeline; `PIPESTATUS=(1 0)` |
| 3 | "`omp-types` re-exports the ack vocabulary" | `CrateSpecs` | read the intent, not the file; `ObligationLedger` occurs zero times |
| 4 | "no prior art for typed missing-dependency" | `EndUserJourney` | `--include='*.rs'` aimed at **ntm, a Go repo** |
| 5 | "no prior art for anti-vacuity" | `GateFrameworks` | search space too narrow — 3 doc files; the concept lives in tests, telemetry, a shell gate, and a Lean proof |

**Three distinct false-zero mechanisms**, none of which look like failure at the call site: a shell
`grep --include=` that returns empty at exit 0; an extension filter pointed at the wrong language;
and a search space too narrow to contain the answer. All three produce a confident *"no prior art
found"* that is **indistinguishable from a real one**.

The rule that fell out of it, now doctrine: **a not-found is publishable only if it names the command
AND argues that the search space could have contained the answer.** *"I grepped and got nothing"* is
not a finding. *"I grepped `*.rs` across a Go repo and got nothing"* is a bug.

A fourth failure was purely editorial and produced its own rule. Three agents cited the same
precedent as `doctor.rs:924`, `:949`, and `:950` — all partly right, because each named a **different
construct on adjacent lines** (the function, the gate, an off-by-one). **A citation must name the
construct, not just the line**: a line number is unverifiable alone and does not survive a reformat.

### 7.1 The three newest gate-admission rules were all paid for by us

`GateFrameworks` observed the thing that matters most here. The gate-admission checklist gained three
items this session — **12** (an exit status is not evidence of a successful probe), **13** (a bare
zero is not a finding), and **14** (cite the construct, not the line) — and **not one came from
auditing a gate**. All three were paid for by the conductor getting it wrong *in the document that
specifies the rule*.

That is the strongest available evidence for the central claim of this plan. We argue that a fleet
needs adversarial re-derivation because self-report is not evidence. The brief asserting that
principle was itself corrected five times by agents applying it — against its author, within hours,
each with a command and a result. The process caught its own author. **An investor should weigh that
more heavily than any green test in this repo**, because it is the only evidence here that the
method works on the person running it.

**NO-CLAIM.** Five refutations in one session is evidence the challenge mechanism functions. It is
**not** evidence that the remaining measurements are correct — only that five specific ones were
wrong and are now recorded. The base rate of undetected errors in this document is unknown and is
not estimated anywhere. Nothing here was found by an automated check; every one was found by an
agent choosing to re-derive, and no gate in this repo enforces that choice.

### 7.2 The rule cannot be held by discipline — measured on the author, twice

Roughly twenty minutes after writing the pipeline-laundering finding into §3.1, the conductor
repeated it. Verifying the `installer` fix, the check was run as
`installer --check 2>&1 | head -10 | ... ; echo "exit=$?"` and reported **`exit=0`** for a binary
that had just printed `INSTALLER IDENTITY DRIFT`. Re-measured without a pipeline:

```
installer --check >out 2>err ; echo $?
  true_exit=1   stdout=347B   stderr=72B
```

The binary is **correct** — it exits 1, puts the drift line on stderr and the data on stdout, a
clean split and an honest code. The measurement was wrong, in exactly the way documented one screen
earlier, by the person who documented it.

**This is the argument for mechanism over vigilance, and it is the only version of that argument
backed by a measurement on the author.** Knowing the rule, having just written the rule, and
actively verifying a fix *for a defect in the same family* was not sufficient to avoid the rule's
own failure mode. A rule that must be remembered at the moment of use will be forgotten at the
moment of use. That is why §7.1's admission items 12–14 are gate items rather than style guidance,
and it is why the honest reading of §7 is not "our process caught five errors" but **"our process
caught five errors and has no mechanism preventing the sixth."**

*Recorded under R11. The corrective is a checker that refuses `$?` after a pipeline in any command
cited as evidence — specified in `09-milestones.md` as PC7, and not built.*

---

**NO-CLAIM.** This brief records requirements and measurements. It does not establish that the plan
satisfies them, that the measurements are complete, or that the sections listed in §5 exist yet at
the quality bar §6 demands. Grading is a separate pass and it has not run.

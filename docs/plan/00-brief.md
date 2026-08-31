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

`tmux` is the one binary in the set with no machine-readable version handshake, and pane truth is
read through it. That is a named dependency risk, not a footnote.

### 3.2 The OMP surface census

Produced by the built scanner: `/Volumes/BuildShared/cargo-targets/debug/omp-inventory-map`
→ 544,697 bytes of JSON, **exit 2**, envelope
`{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"UNKNOWN","data":{…}}`.

- **184 nodes, 207 edges, 183 rows.**
- Counts: `cli_commands=39`, `type_roots=57`, `declarations=14`, `rpc_handlers=42`,
  `slash_commands=0`, `omp_methods=3`, `workspace_crates=26`, `expected_cli_commands=39`.
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

**1 of 8 gates has all four legs.** 5 of 8 have no mutation leg. 2 of 8 have no known-bad. 1 of 8 —
`path-literal-guard` — has **no known-good leg**, which makes it the highest-risk gate in the set:
an attack-only suite ships an over-strict gate, and an over-strict gate gets routed around, which is
a slower death than no gate at all.

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
  fns take `cx` first; 16 of 22 forbid unsafe.
- `omp-types` exists, re-exports the canonical vocabulary from asupersync at the pinned rev
  (`AckKind`, `DeliveryClass`, `ObligationLedger`, `Budget`, `Outcome`) — and has **zero
  dependents**. The vocabulary is shipped and unadopted.
- Type inventory: **51 public enums, 79 structs** across 22 of 24 crates; **4 colliding type
  names**; **6 Verdict-shaped types with no shared trait**; **17 ack/receipt types in 3 incompatible
  dialects**.
- `fh` MCP is failing closed with a typed `SERVE_INPUT_STALE` (mirror HEAD moved `5dec4212…` →
  `ecdea397…`). Direct grep of the mirror at `/Volumes/ZestData/dicklesworthstone-mirror` (216
  repos) still works. **Failing closed with a remediation hint is the model**, not a defect.
- Board at stand-down: **28 closed, 25 in_progress, 19 open, 2 blocked** (75 total).

---

## 4. The four-layer reality — what works and what does not

This is the spine of the whole plan. Exactly one row works today.

| layer | mechanism | measured state |
|---|---|---|
| observe | `tick-monitor` | **WORKS** |
| actionable | `idle_panes` | **BROKEN** — discards `NewlyIdle`; `free_capacity` derives from the same `is_dispatchable` filter, which requires *Confirmed* Idle, so a pane at `t=0` is excluded from **both** lists |
| consume | `decide()` | **FENCED** — 162 refused ticks over 4.2 hours, `DISPATCH_RETRY_BLOCKED` |
| actuate | dispatch | **DOES NOT EXIST** — a human types into panes |
| complete | worker says done | **DOES NOT EXIST** — every completion this session was found by a human looking |

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

**NO-CLAIM.** This brief records requirements and measurements. It does not establish that the plan
satisfies them, that the measurements are complete, or that the sections listed in §5 exist yet at
the quality bar §6 demands. Grading is a separate pass and it has not run.

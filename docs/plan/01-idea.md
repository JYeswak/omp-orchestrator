# 01 — The idea

*Serves R10 (idea → why → binaries → SOTA bar) and R3 (investor-attackable). Written to the writing
contract in `00-brief.md` §6. Every number carries its deriving command; where this section
disagrees with the brief, it says so in the open — §1.3.9.*

---

## 1.1 The one sentence

**`omp-orchestrator` is a typed supervisor that makes an agent fleet's finished work provable — it
wraps the coding-agent toolchain in Rust gates so that "done" is a machine-checked verdict carrying
its own evidence, not a claim in a chat log.**

The load-bearing word is *provable*. There is no shortage of software that starts agents; what does
not exist is an accountant for the ones already running.

---

## 1.2 Why this exists

The problem is not that an agent fleet fails to work. The problem is that it works enormously and
then cannot tell you what it finished.

That is not a thesis, it is a reap. From the stand-down of one real session on this repository
(MEASURED — stand-down reap plus `00-brief.md` §3.7, §4):

- **6 beads landed awaiting grade. ZERO beads landed *and* closed on the day they landed.** Work
  reached the tree; the ledger never learned it had. For every item that day, the gap between "the
  code is in" and "the board says the code is in" went unbridged.
- **7 real conditions were live in the repository and belonged to no bead at all** — they existed
  only in pane scrollback, which dies with the pane. Two were pre-existing red tests hiding in a
  suite *already expected to be red*. An already-red aggregate is perfect camouflage: a new failure
  adds no signal, because the signal was already saturated.
- **162 refused dispatch ticks across 4.2 hours, with a human as the only actuator.** The supervisor
  correctly knew work existed, correctly knew it could not start it, and said so 162 consecutive
  times. Nothing in the system could resolve the refusal except Josh typing.
- **A 23-commit drift between the installed supervisor binary and HEAD.** The thing enforcing the
  rules was 23 commits behind the rules.
- Board at stand-down: **28 closed, 25 in_progress, 19 open, 2 blocked** (75 total). Twenty-five
  simultaneously in-progress items is not throughput; it is unresolved state.

### The mechanism behind the 162

The 162 is the symptom with a named cause, and the cause is the whole argument for typed
supervision. From the five-stage control loop (formerly "five-stage" — renamed, the table has five stages and seven rows) table (`00-brief.md` §4, MEASURED):

| layer | mechanism | measured state |
|---|---|---|
| observe | `tick-monitor` | **WORKS** |
| actionable | `idle_panes` | **BROKEN** — discards `NewlyIdle`; `free_capacity` derives from the same `is_dispatchable` filter, which requires *Confirmed* Idle, so a pane at `t=0` is excluded from **both** lists. OMP declares `GuestIdleReconcilerCtx` (`dist/types/collab/guest.d.ts:9-30`) with the analogous settle/continuation split, but it is DECLARED ONLY and not wired here |
| consume | `decide()` | **FENCED** — 162 refused ticks over 4.2 hours, `DISPATCH_RETRY_BLOCKED` |
| actuate | dispatch | **DOES NOT EXIST** — a human types into panes |
| complete | worker says done | **AVAILABLE, NOT WIRED** — OMP emits `AgentEndEvent.willContinue` on `RpcSessionEventFrame` (`modes/rpc/rpc-types.d.ts:589`); a raw `agent_end` frame with `isTerminal:true` crossed the `--mode=rpc` wire, but this supervisor does not consume it |

Exactly one of five layers works in the local supervisor. Completion is no longer an upstream absence: `AgentEndEvent.willContinue` is WIRE-PROVEN on the `--mode=rpc` channel, but the local loop still has no consumer for it.

Nothing crashed. Nothing threw. A single shared predicate, used to answer two different questions,
produced a coherent and completely wrong world model 162 times in a row, and the only reason anyone
found out is that a human was watching. That is why the answer is *types* and not *more logging*: a
log would have faithfully recorded 162 correct refusals. The defect is that "is this pane
dispatchable" and "how much capacity is free" were ever allowed to be the same question.

Four symptoms, one shape: **no typed answer to a question the supervisor must answer in one call.** What is finished? What is broken that nobody holds? Why did the loop refuse? Is the enforcer current? The completion signal now exists on the wire, but the supervisor still does not attach to it; the other six gap types remain DECLARED ONLY.

**NO-CLAIM:** these counts come from a single session's reap on one machine and one checkout. They
are not claimed representative of any other session, operator, or workload, and no rate, average, or
trend is asserted. They establish that these failures *occurred*, not how often they occur.

### 1.2.1 The seven gap claims — what the upstream types actually change

The upstream sweep changes the strength of the absence claims without pretending that a declared type is a consumed contract. One gap is WIRE-PROVEN; the other six are DECLARED ONLY.

| gap | upstream type and source | true strength | effect on the idea
|---|---|---|
| completion | AgentEndEvent.willContinue + SessionStopEvent (extensibility/shared-events.d.ts:83-93,154-162), on RpcSessionEventFrame (modes/rpc/rpc-types.d.ts:589) | WIRE-PROVEN — raw agent_end with isTerminal:true crossed --mode=rpc | adopt the existing event channel; supervisor integration remains the work
| receipts | IrcDeliveryReceipt + AsyncJobDeliverySink (tools/hub/types.d.ts:8,84) | DECLARED ONLY — no wire path measured | the cp-z42vu transport/receipt gap remains; type existence does not replace receiver proof
| claims | Stage1Claim / GlobalClaim with ownershipToken + inputWatermark (memories/storage.d.ts:20-27) | DECLARED ONLY — no wire path measured | local claim/ownership gap remains until reachability and semantics are proven
| idle | GuestIdleReconcilerCtx (dist/types/collab/guest.d.ts:9-30) | DECLARED ONLY — no wire path measured | the local NewlyIdle/ConfirmedIdle defect remains; the upstream split is corroboration, not a fix
| roster | HubRosterCounts (dist/types/tools/hub/types.d.ts:33-90) | DECLARED ONLY — no wire path measured | hand-derived roster remains an unclosed observation gap
| cost | SearchUsage (dist/types/web/search/types.d.ts:232-254), PerplexityCost (:510-527), ContextUsage (dist/types/extensibility/extensions/types.d.ts:238-240) | DECLARED ONLY — no wire path measured | cost telemetry remains unmeasured
| compaction | SessionBeforeCompactEvent / SessionCompactEvent (dist/types/extensibility/shared-events.d.ts:54-75) | DECLARED ONLY — no wire path measured | context-loss recovery remains unproven; the type narrows adoption work but does not close it

**NO-CLAIM:** The completion result proves a wire frame, not supervisor consumption. The other six entries refute “no upstream type exists” only; they do not prove --mode=rpc reachability, semantic fit, or adoption cost.

---

## 1.3 The binaries we are wrapping

We are not building a runtime. We are building a typed contract layer over binaries that already
exist and are already good. The bet is that the scarce thing is not execution capacity but
*accountability across tools that do not share a vocabulary*.

Measured 2026-08-31 by `<bin> --version` (MEASURED; reproduced from `00-brief.md` §3.1, not
re-derived):

| binary | version | path |
|---|---|---|
| `omp` | `omp/18.0.11` | `/Users/josh/.local/bin/omp` |
| `ntm` | `ntm version v1.30.0-1-gda270719` | `/Users/josh/.local/bin/ntm` |
| `br` | `br 0.4.1` | `/Users/josh/.local/bin/br` |
| `bv` | `bv v0.20.0` | `/opt/homebrew/bin/bv` |
| `git` | `git version 2.50.1 (Apple Git-155)` | `/usr/bin/git` |
| `cargo` | `cargo 1.100.0-nightly (e8cb624d5 2026-08-2…)` | `/Users/josh/.rch/shims/cargo` (a shim; real cargo at `~/.cargo/bin/cargo`) |
| `fh` | `franken-harvest 0.1.0+tree.7b0fc50c3e5a29d…` | `/Users/josh/.local/bin/fh` |
| `jsm` | `jsm 0.1.4` | `/usr/local/bin/jsm` |
| `tmux` | rejects `--version` (`tmux: unknown option -- -`) | `/opt/homebrew/bin/tmux` |

**1.3.1 `omp` — the agent surface.** We call it for the surface itself: CLI subcommands, the
JSON-RPC transport reached through `--mode=rpc`, and the type roots defining what a session can be
asked to do. *Contract depended on:* the versioned envelope and the stability of the `--mode=rpc`
selector. *If it drifts:* every consumer re-derives a different graph from the same repository and
our stored inventory silently describes a surface that no longer exists — a stale map that still
renders. Deepest dependency; per §1.5, least exercised.

**1.3.2 `ntm` — pane truth.** We call it for which panes exist, what is in them, and which are live
agents rather than dead shells. *Contract depended on:* pane identity is stable across the window in
which we dispatch to a pane and then read back from it. *If it drifts:* a receipt binds to the wrong
actor and the ledger records a delivery that never happened — a **false positive on completion**,
precisely the class §1.2 exists to eliminate, arriving through the mechanism meant to eliminate it.

**1.3.3 `br` — the bead board.** We call it for the durable record of what work exists, who holds
it, and what state it is in. *Contract depended on:* `br`'s typed close-policy refusal — it refuses
to close a bead that has not satisfied policy, and refuses *with a type*. That is load-bearing: it
is the upstream mechanism making "landed awaiting grade" a representable state rather than untracked
limbo. *If it drifts* into a warning, the 6-landed-zero-closed measurement stops being *unresolved*
and becomes *invisible*, which is strictly worse.

**1.3.4 `bv` — triage.** We call it for ranking what to work next. *Contract depended on:*
`--robot-triage` returning multiple ranked slices in one call rather than N round-trips. That is not
ergonomics; it is what makes a supervisor tick cheap enough to run on a loop. *If it drifts* to
one-slice-per-call, tick cost multiplies by slice count and cadence collapses. Jeffrey's own agent
handbook states the same as a safety rule — `beads_rust/src/cli/commands/robot_docs.rs:59`: *"Avoid
bare `bv` in automated sessions; use `bv --robot-*` flags."* The robot surface is the contract; the
human surface is not.

**1.3.5 `git` — tree ground truth.** We call it for what is committed, what is dirty, and what
`git ls-files` says is tracked. *Contract depended on:* `git ls-files` is the authoritative
tracked-file set — our one hard rule (no `.sh`, no `.py`, exemption list empty) is enforced by a Rust
gate walking exactly that set. *If it drifts,* or if we ever substitute a filesystem walk, an
untracked scratch script passes and a hard rule silently becomes advisory.

**1.3.6 `cargo` — build and metadata, through a shim.** We call it for build and for
`cargo metadata --format-version 1 --no-deps`, the flagged versioned form our crate census reads.
Note the measured path: `cargo` resolves to `/Users/josh/.rch/shims/cargo`, **a shim**, with real
cargo at `~/.cargo/bin/cargo`. A shim in the dependency path is a supply-chain surface — it can
rewrite arguments, change the effective toolchain, or add latency — and the census attributes results
to "cargo" without qualification. **Every crate-metadata claim inherits whatever the shim does, and
we have not measured what it does.** Open item, recorded here rather than left in chat (R11).

**1.3.7 `fh` — evidence, failing closed.** We call it for retrieval against the dicklesworthstone
mirror — 210 git work-trees at `/Volumes/ZestData/dicklesworthstone-mirror` — which is how R7 gets
answered with a citation instead of a memory. *Contract depended on:* `fh` **fails closed with a
type**. As of this session its MCP surface returns `SERVE_INPUT_STALE` because the mirror HEAD moved
(`5dec4212…` → `ecdea397…`); direct grep still works, and this section's citations were taken that
way. **That failure is the good outcome** — it refused to serve a stale index rather than answering
confidently from data it knew was behind. *If it drifted* to best-effort, every prior-art citation in
this plan would become unfalsifiable, which is worse than having none, because unfalsifiable
citations survive review.

**1.3.8 `jsm` — skill resolution.** We call it for skill resolution and installation topology.
*Contract depended on:* the single-store-plus-symlinks invariant; the session-start gate reports
`PASS (one store, N symlinks)`. *If it drifts,* two divergent copies of a skill become simultaneously
resolvable and behaviour starts depending on resolution order rather than content — a nondeterminism
that reproduces only on the machine holding both.

### 1.3.9 `tmux` — and a measured disagreement with the brief

We call tmux as the substrate under `ntm`; pane truth is ultimately read through it.

**This section disagrees with `00-brief.md` §3.1 (lines 113–114),** which states: *"`tmux` is the one
binary in the set with no machine-readable version handshake."* Per the writing contract the
disagreement is named, not quietly reconciled.

- *Fact challenged:* tmux has no machine-readable version handshake.
- *Command run:* `tmux -V; echo "exit=$?"`
- *Result (MEASURED):* `tmux 3.6a` / `exit=0`.
- *Also confirmed (MEASURED):* `tmux --version; echo "exit=$?"` → `tmux: unknown option -- -`, usage
  block, `exit=1`. The brief's table row is correct; its interpretive sentence is not.
- *Disagreement:* tmux **does** have a machine-readable version handshake. It rejects the GNU
  long-flag convention the other eight binaries accept, and answers on `-V`.

The corrected risk is narrower than the brief's framing, and worth more precisely because it is
narrower. The danger is not "we cannot learn tmux's version." It is that **a uniform probe loop
written against `--version` records tmux as absent or broken rather than present at 3.6a** — a
dependency checker treating nonzero exit as "missing" will mis-report the one binary through which
we read pane truth, and will do so with a confident negative. That is a false-negative-on-presence,
the mirror image of the false-positive-on-completion in §1.3.2. We will special-case tmux to `-V`
and comment the special case in code, because an unexplained special case is exactly the knowledge
the next refactor deletes.

**NO-CLAIM:** the table records what resolved on `PATH` at one moment on one host (Apple M3 Ultra,
darwin 25.5.0). It does not claim these versions are pinned, reproducible elsewhere, free of the
`cargo` shim's effects, or that the stated contract per binary exhaustively describes that tool's
interface. "If it drifts" clauses describe consequences we reason about; none has been observed and
none is a prediction of likelihood.

---

## 1.4 What SOTA means here, and why the bar sits where it does

R10, verbatim: *"every aspect of this needs to be on par or greater than SOTA - same as the binaries
we are wrapping."*

That is only useful if operational. So: for each wrapped binary, name the **specific property it
already ships** that we must match or beat. These are not aspirations — they are existing behaviours
in tools we invoke daily, which makes the bar empirically reachable. Someone already reached it, on
this machine, in this toolchain.

| wrapped binary | property it already ships | our obligation |
|---|---|---|
| `bv` | `--robot-triage` returns **multiple ranked slices in one call** | a tick answers multi-part questions in one invocation, never N |
| `br` | **typed close-policy refusal** — refuses to close, with a type | gates refuse with a typed reason, never a bare nonzero exit |
| `fh` | **fails closed** with typed `SERVE_INPUT_STALE` + remediation hint | every surface prefers typed refusal over a confident stale answer |
| `omp` | **versioned envelope** on output | every artifact carries `schema_version` |
| `git` | `ls-files` is an **authoritative machine-parseable set** | gates enumerate from an authority, never a filesystem walk |
| `jsm` | **single-store invariant, checked at session start** | installation topology is checked, not assumed |
| `tmux` | machine-readable version at `-V`, exit 0 | our binaries answer a version probe **on the conventional flag** — we do not ship tmux's asymmetry ourselves |

**Where we already meet the bar (MEASURED).** The built scanner
(`/Volumes/BuildShared/cargo-targets/debug/omp-inventory-map`) emits
`{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"UNKNOWN","data":{…}}` and
exits **2** on `UNKNOWN`, producing 544,697 bytes. Versioned envelope, plus a distinct exit code for
the uncertain case. It matches `omp` on envelope and beats a boolean exit on refusal semantics,
because `UNKNOWN` is not conflated with `FAIL` — and a supervisor that cannot distinguish "this is
broken" from "I could not tell" will eventually act on the second as if it were the first.

**Where we are below the bar — a named defect (MEASURED).** `omp-inventory-map --help` returns:

```json
{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"ERROR",
 "data":null,"error":"CONFIG_ERROR unknown argument --help"}
```

The gate is **built, correct, and undiscoverable.** Thirteen tests pass;
`crates/omp-inventory-map/src/types_inventory.rs:176-178` deliberately excludes `Observation` from
the allowance list so a name collision *demands* convergence rather than being waved through — and
the running binary's 544 KB output contains **zero** occurrences of `Observation`, `CONVERGE`, or
`Verdict`. This is not built-versus-wired. It is **wired-but-unaddressable**, a worse diagnosis,
because it survives every check that asks "does the code exist and pass."

Note the shape, because it recurs in §1.5: `--help` did not return a generic error. It returned a
*typed* error (`CONFIG_ERROR`) with an accurate message. **The typing discipline was applied
perfectly, to the wrong outcome.** Rigor pointed at the wrong target is this codebase's
characteristic defect. Hence a sixth required gate property, now first-class across the plan:

> **ADDRESSABLE** — one documented command runs the gate, and `--help` names that command.

### What would Jeffrey do — prior art for ADDRESSABLE (R7)

Searched the mirror for self-describing CLI surfaces (`robot-docs|robot_docs` and
`CONFIG_ERROR|unknown argument` across `/Volumes/ZestData/dicklesworthstone-mirror/beads_rust` and
`.../ntm`). **Prior art found**, and it is stronger than the property we named:

1. **Discoverability is a shipped command, not a flag.** `br robot-docs guide` is a top-level
   subcommand whose entire job is machine-readable self-documentation, carrying its own contract
   version: `const CONTRACT_VERSION: &str = "br.robot_docs.v1";`
   (`beads_rust/src/cli/commands/robot_docs.rs:11`). The doctor subsystem ships its own,
   `br.doctor.robot_docs.v1` (`.../doctor_subsystems/surface.rs:108`). Discoverability is versioned
   like any other output.
2. **CI enforces that discoverability keeps working.** `beads_rust/.github/workflows/doctor.yml:88-93`
   runs `br doctor robot-docs --format json` and asserts both
   `jq -e '.schema_version == "br.doctor.robot_docs.v1"'` and `jq -e '.line_count > 20'` — the second
   an anti-vacuity check on the docs themselves: present *and non-trivial*.
3. **The help surface must not fail for the same reason the tool fails.**
   `beads_rust/src/main.rs:3744-3747` asserts `!needs_write_lock(&robot_docs)` with the message
   *"robot-docs is a pure help surface and must not depend on workspace lock health."* The deepest of
   the three, and we had not stated it: a discoverability surface that degrades when the workspace
   degrades is unavailable exactly when it is needed.
4. **Contract drift is a P0 bug filed against the docs, not the code.** Bead `beads_rust-mbpq`
   (`beads_rust/.beads/issues.jsonl:697`) was raised *because* `capabilities` and `robot-docs` claimed
   a concurrent `--repair` exits 5 on lock contention while the binary silently waited 30s and exited
   0 — filed *"P0 because it contradicts a documented contract."* The documented contract is normative
   over the implementation.
5. **Exit codes are a dictionary, not a convention.** Bead `beads_rust-rw4u`
   (`.../issues.jsonl:843`) specifies eleven variants: `0` healthy, `1` findings present, `2` fix
   partial, `3` fix failed and rolled back, `4` refused unsafe, `5` concurrency lost, `6` online
   required, `64` usage error, `66` no input, `73` cannot create output, `74` I/O error — and CI
   asserts `(.exit_codes | length) >= 11` (`doctor.yml:85`). Stated rationale: returning 0 with
   warn-level findings forces CI to parse JSON to gate.

**This raises our bar, and we adopt it (PROJECTED).** ADDRESSABLE as first stated — "`--help` names
the command" — is the weakest form. The mirror's form is a *versioned, CI-asserted, lock-independent*
self-documentation surface plus an exit-code dictionary rich enough that a caller branches without
parsing a payload. Our single distinction (`2` = UNKNOWN) is directionally right and
dictionary-poor. §06 owns the resulting spec; §10 owns the full prior-art pass.

**NO-CLAIM:** this names properties observed in these tools' documented interfaces and, where cited,
in mirror source at a specific file and line. It does not claim those properties exhaustively
describe those tools, that they hold across versions, that the mirror checkout matches upstream HEAD
(it does not — see `SERVE_INPUT_STALE`, §1.3.7), or that matching them is *sufficient* for quality.
Matching a bar is a floor-raise, never a guarantee.

---

## 1.5 The honest position

**What is genuinely built (MEASURED).** Twenty-six workspace crates. **379 `#[test]` functions across
26 integration test files** (`find crates -name '*.rs' -path '*/tests/*' | wc -l` → 26;
`grep -rc '#\[test\]'` → 370). A working census: **184 nodes, 207 edges, 183 rows**, 544,697 bytes,
including a complete 18-edge dependency graph in which `subprocess-contract` is correctly the
most-depended-on crate at 4 dependents — the right shape, since it is the asupersync process-group /
drain-both-pipes boundary and everything that spawns should route through it. An `omp-types` crate
correctly re-exporting the reachable part of the canonical async vocabulary (`Budget`, `Outcome`; **not** `AckKind`/`DeliveryClass`/`ObligationLedger`, which are blocked upstream — corrected, see brief §3.7),
`Budget`, `Outcome`) from asupersync at pinned rev `fa3c01aec`. A hard no-`.sh`/no-`.py` rule enforced
by a Rust gate over `git ls-files` with an empty exemption list. This is not a prototype.

**The sentence that undercuts all of it (MEASURED).** Of the 26 workspace crates, **25 consume zero
OMP surface.** All 7 `consumes` edges in the entire 207-edge graph originate from one crate —
`omp-inventory-map`, the scanner — each carrying the evidence string *"direct process probe produced
this row."* The scanner consumes OMP because the scanner's job is to look at OMP. Nothing else does.

**The thing named `omp-orchestrator` does not yet speak to OMP.**

### The three objections an investor should raise

Rule 8 of the writing contract: state the strongest form, then answer or concede.

**Objection 1 — "You have built 26 crates of scaffolding around a hole. The one integration that justifies the name does not exist."** *Partly conceded, with a narrower truth.* The completion signal is now WIRE-PROVEN upstream, but the supervisory integration that would consume it still does not exist. The 25-of-26 measurement is ours, not a reviewer’s.
reviewer's. The partial answer is that the layer census shows `observe` WORKS and failure is
concentrated in `actionable`/`consume`/`actuate` — but we will not lean on it, because "one of five
layers works" is not a rebuttal. What we do not concede is that the crates are therefore waste: the
gates operate on the repository and the process boundary, not on OMP, and they run today. The
accurate statement is that the *supervisory* path is unbuilt while the *enforcement* path is built
and running.

**Objection 2 — "Your own evidence discipline is theatre."** *Conceded outright, and we found it
ourselves.* All 183 census rows carry the four mandatory fields — `inputs`, `outputs`, `must_be_true`,
`negative_evidence` — with **zero missing**, and exactly **one distinct value** of `must_be_true` and
**one distinct** `negative_evidence` across the entire census (`00-brief.md` §3.3;
`python3 -c "…Counter(json.dumps(r.get('must_be_true')) for r in rows)…"` → crate rows n=26,
distinct=1; non-crate rows n=157, distinct=1). For the 26 crate rows, `inputs`/`outputs` describe
*the scanner's own provenance* rather than the crate's contract, and `what_it_provides` is
boilerplate — "Workspace crate X from cargo metadata" — distinct 26 ways only because the name varies.
The four-field discipline this orchestrator demanded of every worker was satisfied **syntactically
and vacuously**, and the indictment lands on the conductor, not the workers. The answer is
structural: every gate must ship an **anti-vacuity** leg, and the success criterion is not "the
fields are present" but "**the field values discriminate**." A schema fully populated with one value
carries exactly zero bits.

**Objection 3 — "Your gates are unevenly built, so your floor is the weakest one."** *Accepted as
stated.* Of 8 gates, **2 have all four legs** — `no-shell-gate` (known-bad 4, known-good 3, mutation
2, anti-vacuity 6) and `undrained-pipe-lint` (1/1/1/3). **4 of 8 have no mutation leg**, 2 of 8 have
no known-bad, and `path-literal-guard` has
**no known-good leg at all** (`00-brief.md` §3.5, recomputed — the brief's first draft said "1 of 8"
and "5 of 8" and both were refuted by its own table). That is the highest-risk gate in the set, and
not because it is small: an attack-only suite ships an over-strict gate, an over-strict gate gets routed
around, and a routed-around gate is a slower death than no gate — the routing is undocumented and the
gate still reports green.

### The corollary connecting all three

`omp-types` — the crate holding the canonical vocabulary — has **zero dependents**. The vocabulary is
defined and unused. That is not a tidy-up item; it is the direct cause of the type inventory
measuring **51 public enums (excluding test+bin sources; 59 including them — publish the pair) and 79 structs across 22 of 24 crates**, with **6 distinct Verdict-shaped
types sharing no trait**, **17 ack/receipt types in 3 incompatible dialects**, and **4 colliding type
names**. Six ways to say "verdict" and three ways to say "acknowledged" is exactly how a fleet ends a
session unable to state what it finished. The vocabulary problem and the accounting problem in §1.2
are the same problem seen at two altitudes.

**NO-CLAIM:** this reports one checkout on 2026-08-31. It does not claim the census is complete, that
183 rows enumerate every OMP surface that exists, that the gate-leg counts (derived by `grep -rli`
per property) correctly classify every test's intent, or that the type-collision counts are
exhaustive. Grep-derived leg counts detect *naming*, not *semantics*: a test named for a property it
does not exercise counts as present here. Re-deriving these under a stricter method is a separate
pass and has not run.

---

## 1.6 Recorded under R11

Two constraints surfaced while writing this section that were written down nowhere, and are recorded
here rather than left in conversation:

1. **The `cargo` shim is an unmeasured supply-chain surface** in the metadata path (§1.3.6). We know
   it is a shim; we have not measured its effect on arguments or toolchain selection.
2. **The brief's tmux interpretation is wrong, and the real risk is a false negative on presence**
   (§1.3.9) — so no dependency-probe loop we write may treat nonzero exit as absence.

**NO-CLAIM:** nothing here commits to a schedule, a cost, or an architecture validated by execution.
This section establishes what exists, what is measured, what bar we hold ourselves to and why it is
reachable, and where we currently fall below it. Whether the plan that follows discharges any of it
is decided in §09, by a grading pass that has not run.

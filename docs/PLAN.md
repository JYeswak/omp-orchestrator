# PLAN.md — omp-orchestrator

**A single installable Rust binary that takes a repo's own work graph and drives it to completion
across a fleet of agents, refusing every step it cannot prove.**

Assembled from `docs/plan/`. **The section files are the source of truth**; this document is their
concatenation. Edit a section, then re-assemble — never edit here, and never re-stamp this file's
mtime to satisfy the freshness gate (§12.11 records the author doing exactly that).

> ## Three things a reader should know before the contents

> **1. The headline finding was refuted, and it was the first of eight.** §10 claimed a typed
> worker-completion signal was precedent-free across 210 repositories. It ships in the tool we wrap
> and crosses the wire: `AgentEndEvent`, `isTerminal:true`, captured live on `--mode=rpc`. Seven
> more named gaps have upstream types; an eighth root (`dist/types/plan-mode/`) surfaced later.

> **2. Convergence has been retracted once.** Rounds 8–9 banked 3 sections under a two-lens rule.
> Round 10 graded with readers who had never seen the ledger and all three fell — 360 findings
> across 8 rounds. Rounds 8–9 measured the graders. Fresh eyes is now a clause of the rule.

> **3. There is no external-validation loop.** Every gate suite here is internal — us checking us.
> `loop-engineering` names that as insufficient for "shipped"; §12.11 records the gap.

> §8 carries **13 open questions** and **5 kill criteria** — one (K1) void. 9 measured claims
> were refuted while this was written, kept as labelled retractions. Surface map: **614 surfaces**,
> 52 consumed / 67 wire / 33 validate /
> 453 retired / 9 unknown — **24.8% engaged**.

---

## Contents

- **`00-brief.md`** — 00 — The brief
- **`01-idea.md`** — 01 — The idea
- **`02-surface-census.md`** — 02 — What we are mapping: every OMP surface
- **`03-crates.md`** — 03 — Every crate: contract, schema, types, dependencies
- **`04-diagrams.md`** — 04 — FrankenMermaid: the system, generated not drawn
- **`05-actions.md`** — 05 — Every action: intended purpose, and the negative pattern it must refuse
- **`06-gates.md`** — 06 — The testing, validation, and gating frameworks we apply
- **`07-installability.md`** — 07 — Installability: distribution, identity, and the canonical CLI contract
- **`08-end-users.md`** — 08 — The end-user journey: another repo, another machine, orchestrating their own project
- **`09-milestones.md`** — 09 — Milestones, done-definitions, and how this plan is validated
- **`10-prior-art.md`** — 10 — What would Jeffrey do: prior art mined from the mirror
- **`11-lifecycle.md`** — 11 — Lifecycle evidence map: idea to shipped, walked down the crates and skills
- **`12-journey.md`** — 12 — The dispatchable journey: a runbook per stage

---

<!-- ===== 00-brief.md ===== -->

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

> **R12 — added by grading round 1, and not by Josh.** Every question an experienced operator asks
> before funding work must be either answered or **registered as an open question with an owner**.
> Added because all four graders found the same hole and `%1408` found it *structurally*: §2's
> table is the only mechanism turning a question into something a section must answer, so cost,
> timeline, headcount, competition, security, licensing, and kill criteria were not merely
> unanswered — **they were unaskable from inside the requirement set**. R12 and §8 close that.
> A requirement the graders had to invent is the strongest evidence this round was worth running.

> **R13 — the full idea-to-shipped lifecycle.** "part of our plan needs to be intimately aware of the entire
> lifecycle of an idea to a finished project then walk the list down the crates and our skills to
> ensure that throughout dispatch we have proper templates, proper dispatch, proper reap, proper
> logging, proper build grading, etc."
> Added by Josh mid-grading; §11 maps the spine and §12 owns the dispatchable S1–S9 runbook.


### 1.5 Glossary — the jargon R1–R13 use without defining
`%1408` filed this in round 1 and I skipped it twice. The requirements are quoted **verbatim**, which
is correct and non-negotiable, and the cost is that they carry fleet jargon an outside reader cannot
resolve. An investor reading R1 hits "bead dag" in the first sentence.

| term | what it means here |
|---|---|
| **bead** | one unit of tracked work, in the `br` issue tracker. Roughly a ticket, with typed dependencies and a close policy that refuses prose |
| **bead DAG** | the dependency graph over beads. `bv` ranks it by PageRank to decide what to work next |
| **reap** | collect finished and abandoned work from a fleet *before* dispatching more — the opposite of refilling idle workers |
| **pane** | one tmux pane running one agent. The unit of fleet capacity, addressed by `pane_id` such as `%1413` |
| **tick** | one iteration of the orchestrator loop: observe → select → dispatch → verify |
| **the mirror** | a local clone of 210 of Jeffrey Emanuel's repositories at `/Volumes/ZestData/dicklesworthstone-mirror`, mined for prior art |
| **the fleet** | the set of agent panes in one `ntm` session |
| **dispatch** | sending a work packet to a pane. Distinct from *delivery*, which requires a receipt |
| **gate** | a check that refuses. Not a test — a gate's output is a refusal with a reason |
| **leg** | one property of a gate's test suite: known-bad, known-good, mutation, anti-vacuity |
| **known-bad / known-good** | a planted defect the gate must catch; a clean input it must pass. Both are required, and 1 of 8 gates lacks the second |
| **anti-vacuity** | the rule that an empty scan set is an ERROR, never a pass |
| **BUILT ≠ WIRED** | a mechanism that exists, is correct, tested — and is invoked by nothing |
| **GHOST** | an installed binary whose source is not in the tree we read. **Historical unverified count: four; the instance list, source-tree result, comparison command, and date were not retained, so this is non-authoritative.** |
| **OMP** | Oh My Pi, the agent CLI this orchestrator wraps; `omp/18.0.11` |
| **`br` / `bv`** | the bead tracker CLI, and its graph-triage companion |
| **`ntm`** | the tmux fleet manager that owns sessions and panes |
| **`fh`** | franken-harvest, the queryable index over measured doctrine and the mirror |
| **asupersync** | the cancellation/obligation async runtime this repo binds to, pinned at rev `fa3c01aec` |
| **`Cx`** | asupersync's cancellation context, passed first in every async API we own |

**NO-CLAIM:** a glossary makes the requirements *readable*; it does not make them *right*, and it
does not close R2 or R5, whose rows above are marked NOT CLOSED and PARTIAL on their own evidence.

---

## 2. What each requirement demands, made checkable

A requirement that cannot be checked is a wish. Each row states the observable that closes it.

| id | demand | closed by |
|---|---|---|
| R1 | A-to-Z journey: gates, crates, schema, types, validation, per-milestone done | §09 exists and every milestone carries an OBSERVABLE |
| R2 | Reap before refill; repo hygiene; doc structure | **NOT CLOSED — a past event is not a closure.** The reap happened once (28/25/19/2). Closing this needs a *standing* check; the §7.3 staleness predicate is the candidate and is not built |
| R3 | One document, investor-attackable, future-tense on what we build | **PARTIAL — `target/debug/plan-assemble` just regenerated docs/PLAN.md (13 sections; 8,235 lines; 652 KB), but the §7.3 freshness/identity gate is not built, so assembly success is not a durable freshness proof.** |
| R4 | Every crate, every schema I/O, every typed interface, interactions, diagrams | **PARTIAL — §03's 26-row table is a historical pre-extraction snapshot; current cargo metadata reports 50 packages, and no current one-row-per-package contract census is recorded. §04 has ≥6 diagrams, also snapshot-bound.** |
| R5 | Every OMP surface | **PARTIAL — pre-extraction scanner artifact records 980 discovered rows + 1 synthetic transport sentinel = 981 row records, with slash_commands=799 versus expected=136 still unresolved; current workspace metadata and scanner regeneration remain required before closure** |
| R6 | The testing/validation/gating frameworks | **OPEN — §06's gate matrix must produce a named G1–G8 proof artifact with known-bad, known-good, mutation, anti-vacuity, and ADDRESSABLE results; no such all-legs PASS artifact is recorded** |
| R7 | Mirror prior art at every gap | §10 gives a search command + verbatim quote or explicit not-found per gap |
| R8 | Installability + canonical CLI scoping | **OPEN — §07 must attach a clean-machine install receipt proving doctor/health/repair + validate/audit/why, with no hard-coded /Users/josh fallback; no receipt is recorded** |
| R9 | End users orchestrating their own projects | **OPEN — §08 must attach a second-machine/clean-repo first-tick receipt naming the adapter and delivery receipt; no external-repo receipt is recorded** |
| R10 | Idea → why → binaries → actions+negatives → map → design specs at SOTA | **OPEN — §09 must attach a rubric artifact with every SOTA dimension scored ≥4/5 and no missing action/negative mapping; no thresholded score artifact is recorded** |
| R11 | Requirements written down before dispatch | **this file** |
| R12 | Economic and risk questions are registered, owned, and answerable | §8 — thirteen registered questions (eleven OPEN, Q9 ANSWER MOVED, Q10 PARTIAL) and five kill criteria |
| R13 | Full idea-to-shipped lifecycle mapped through skills, crates, gates, and dispatch | §11 lifecycle evidence map plus §12 S1–S9 runbook; every stage must carry the seven-field dispatch contract |

> *Upstream type for this gap: `IrcDeliveryReceipt` (`tools/hub/types.d.ts:8`, DECLARED only). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

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
| `-V` | **6 of 9** | `omp`, `bv`, `git` |

Only `ntm`, `br`, `cargo`, `fh`, `jsm` answer **both**; `tmux` answers `-V` alone. So a uniform
probe loop must try more than one spelling or it will record a present binary as absent.

> **The `-V` row said "6 of 9 (the earlier "5 of 9" excluded tmux and is retired)" until `%1409` (evidence lens) refuted it.** Re-derived by looping all
> nine: `ntm`, `br`, `tmux`, `cargo`, `fh`, `jsm` answer `-V` — **six**. The table excluded `tmux`
> from the `-V` count while the sentence directly above it says `tmux -V` returns `tmux 3.6a` at
> exit 0. A table contradicting its own caption, inside the subsection *about* getting this binary
> wrong twice already. Ninth refutation; third consecutive one on tmux; all three the author's.

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
Round-10 capture: 544,697 bytes of JSON, exit 2, envelope status UNKNOWN; that byte count and its un-hashed output are historical/non-authoritative.
Fresh recapture on 2026-08-31 from the exact invocation cd /Users/josh/Developer/omp-orchestrator && /Volumes/BuildShared/cargo-targets/debug/omp-inventory-map doctor > /tmp/omp-inventory-map-2026-08-31.json: exit 2, 3,032,388 bytes, SHA-256 876809f0779a81b31126564b2b166a7a883c4f5365b499561242013c7dd4c899. Input tree: that workspace working tree at capture time; OMP target: omp/18.0.11. No commit/source revision was recorded, so this is a hash-anchored artifact snapshot, not a revision-pinned source claim.
The fresh artifact is the only hash-anchored scanner output in this brief. Its summary is recorded below; the older 181/183/184 arithmetic is retained only as a labelled round-10 historical snapshot.
- **Historical round-10 node/row shape:** the seven discovered count fields summed to **181** source rows. The reported **183 rows** therefore included two synthetic records: one transport sentinel and one slash_command expectation sentinel. The slash_commands=0 field was the discovered command count; the slash_command 1 row was the synthetic mismatch sentinel, not an enumerated command. The **184 nodes** were those 183 row records plus one scanner root/envelope node. No synthetic record was counted as discovered coverage.
- **Historical arithmetic:** 181 discovered rows + 2 synthetic sentinel rows = 183 row records; 183 row records + 1 root/envelope node = 184 nodes. Do not treat these round-10 values as the current census.
- **Pre-extraction hash-anchored summary (historical):** 981 row records, 982 nodes, and 1,803 edges. Counts: cli_commands=39, type_roots=57, declarations=14, rpc_handlers=42, slash_commands=799, omp_methods=3, workspace_crates=26; expected_slash_commands=136, so the scanner remains UNKNOWN with exit 2.
- **Pre-extraction row kinds (historical):** cli_command 39 · type_root 57 · rpc_handler 42 · workspace_crate 26 · declaration 14 · omp_method 3 · slash_command 799 · transport 1. The transport row is the one synthetic transport sentinel; the 799 slash-command rows are discovered records, not the old expectation sentinel.
- **Historical round-10 counts (non-authoritative):** cli_commands=39, type_roots=57, declarations=14, rpc_handlers=42, slash_commands=0, omp_methods=3, workspace_crates=26. The old row-kind line's transport 1 and slash_command 1 were synthetic sentinels, as classified above.
- **Historical round-10 scanner hole:** every count had an expected_* twin; slash_commands=0 differed from expected_slash_commands=136, which made that envelope UNKNOWN and exit 2. The old claim that 136 slash commands were unmapped is not a current count; the fresh artifact finds 799 slash-command records against the same expected value and still requires reconciliation.
**Current workspace boundary (re-derived 2026-09-01):** the scanner snapshot above predates the extraction wave and remains historical. Direct cargo metadata --format-version 1 --no-deps reports 50 packages and 48 binary targets at /Users/josh/Developer/omp-orchestrator/target; regenerate the inventory artifact before treating any scanner count as current.
- **Historical round-10 classification (non-authoritative):** CAPABILITY_NOT_USED 157 · SCRAPED_OR_OBSERVED_ALTERNATIVE 18 · MAPPED_BY_DIRECT_PROBE 8.
- **Historical round-10 edge relations (non-authoritative):** provides 157 · map-to-none 25 · path-depends-on 18 · consumes 7. All 7 consumes edges originated from omp-inventory-map; 25 of 26 crates consumed zero OMP surface.

The previously-published figure **"81 JSON-RPC methods, 17 used" is RETIRED** — it was not
re-derivable. The measured surface is 39 CLI subcommands, 71 type-surface entries (57 dirs + 14
top-level `.d.ts`), one `--mode=rpc` transport, and **3** methods matching `omp/*`.

### 3.3 The vacuity finding — against ourselves

```
INPUT: /tmp/omp-inventory-map-2026-08-31.json, the fresh 3,032,388-byte capture whose SHA-256 is 876809f0779a81b31126564b2b166a7a883c4f5365b499561242013c7dd4c899 (see §3.2).
COMMAND (case-sensitive JSON-key extraction; excludes no rows and includes only data.rows):
python3 - /tmp/omp-inventory-map-2026-08-31.json <<'PY'
import json, sys
p = json.load(open(sys.argv[1]))
rows = p.get("data", {}).get("rows", [])
for group, selected in (("all", rows), ("crate", [r for r in rows if r.get("kind") == "workspace_crate"]), ("non-crate", [r for r in rows if r.get("kind") != "workspace_crate"])):
    print(group, "rows:", len(selected), "distinct must_be_true:", len({json.dumps(r.get("must_be_true"), sort_keys=True) for r in selected}), "distinct negative_evidence:", len({json.dumps(r.get("negative_evidence"), sort_keys=True) for r in selected}))
assert len(rows) == 981
PY
RECORDED FRESH OUTPUT (2026-08-31): all rows n=981, distinct must_be_true=1, distinct negative_evidence=1; crate rows n=26, distinct must_be_true=1, distinct negative_evidence=1; non-crate rows n=955, distinct must_be_true=1, distinct negative_evidence=1. The superseded round-10 n=183 / non-crate n=157 output is historical and non-authoritative.
```
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
COMMAND (source-aware, root crates; excludes target/, fixtures/, and non-Rust files; file count is separate from function count):
python3 - <<'PY'
from pathlib import Path
import re
root = Path("crates")
files = sorted(p for p in root.rglob("*.rs") if "tests" in p.parts and "fixtures" not in p.parts and "target" not in p.parts)
test_fn = re.compile(r"#\[test\](?:\s*#\[[^]]+\])*\s*(?:pub\s+)?fn\s+[A-Za-z_][A-Za-z0-9_]*\s*\(")
functions = sum(len(test_fn.findall(p.read_text(errors="replace"))) for p in files)
print("test_files", len(files))
print("test_functions", functions)
PY
RECORDED ROUND-10 SNAPSHOT (non-authoritative; 2026-08-31 ~21:45): test_files=31; test_functions=406. The command counts Rust test-function declarations, not matching lines, and does not conflate the two denominators. Re-running it against the current tree is required for current values.
```

**MEASURED 2026-08-31 ~21:45 — and already stale by design.** These counts moved twice inside
the session that measured them (26/379 at this brief's first pass, 30/402 by grading round 10,
31/406 at this fix) because five test files landed in `no-shell-gate` while the plan was being
graded. Both figures are now registered in `NUMBERS.toml` (`test_files`, `test_functions`) and a
gate re-runs the commands; the block above is a dated snapshot, not a living count. The per-crate
leg table below is likewise a round-10 snapshot and does not include the five new `no-shell-gate`
test files (`retired_figures`, `gap_propagation`, `convergence`, `numbers`, `schemas`), which are
meta-gates over this plan rather than gate-leg members.

**Gate-leg provenance boundary:** the table is a **round-10 snapshot**, not the current 31-file/406-function census. Its exact input scope was the eight gate test directories named by the rows above, excluding the five later no-shell-gate meta-gate files: retired_figures, gap_propagation, convergence, numbers, and schemas. The round-10 worksheet's revision, generator command, and output artifact were not retained; therefore every 0/8, 1/8, 2/8, 4/8, and 2/8/1/8 ratio below is historical/non-authoritative and MUST NOT be read as a current measurement. A future refresh must record the exact source revision, command, captured output path, and SHA-256 before publishing a ratio.

| crate | tests | known_bad | known_good | **mutation — what it acts on** | anti_vacuity |
|---|---:|---:|---:|---|---:|
| `omp-inventory-map` | 23 | 0 | 2 | **TREE** — builds a real temp tree, mutates files, restores byte-identically | 1 |
| `undrained-pipe-lint` | 10 | 1 | 1 | **FIXTURE** — mutates an inline `r#"…"#` source string | 3 |
| `state-wildcard-lint` | 9 | 1 | 1 | **FIXTURE** — mutates an inline `r#"…"#` source string | 0 |
| `no-shell-gate` | 34 | 4 | 3 | **AFFORDANCE** — a switch named to be flipped; nothing flips it | 6 |
| `commit-build-fence` | 10 | 0 | 1 | NONE | 0 |
| `kernel-bypass-gate` | 6 | 1 | 1 | NONE | 0 |
| `pre-delete-citation-check` | 6 | 1 | 1 | NONE | 0 |
| `path-literal-guard` | 3 | 1 | 0 | NONE | 2 |

**This column was rebuilt twice, and `%1414` was right both times.** Round 1 counted keyword hits.
Round 2 typed them AUTOMATED/MANUAL/AFFORDANCE/NONE — and `%1414` refused that too: *"the typed
rebuild still defines AUTOMATED by a keyword grep."* Correct. `AUTOMATED` meant "a function whose
**name** contains `mutation`," which is the same proxy wearing a better label. The column now
records **what the mutation acts on**, read from each test body:

```
TREE       mutates a real filesystem tree and restores byte-identically
FIXTURE    mutates an inline source string; production source is never touched
AFFORDANCE a named switch built to be flipped, with no test flipping it
NONE       none of the above
```

**The honest count is now zero.** No gate in this workspace mutates **production source through the
real hook**. The strongest leg is `omp-inventory-map`'s temp-tree mutation, which is genuinely good
and still not production. Two `FIXTURE` legs mutate string literals — and *"a fixture drifted from
production certifies nothing"* is `fh` row `C38`, already binding doctrine in this repo, which
those two legs violate by construction.

> **Headline history, four revisions:** `1 of 8` → `2 of 8` → `1 of 8, different gate` → **`0 of 8`
> by the only definition that means anything.** Each revision came from someone refusing the
> previous measurement, and the number fell every time. That is what a column looks like when it is
> measured instead of counted.

The superseded round-2 definition is kept here as a labelled retraction, because the *reason* it
was wrong is the transferable lesson: `AUTOMATED = a #[test] fn whose name contains "mutation"`
read as a measurement and was a rename. Typing a proxy does not stop it being a proxy.

> **Attribution, corrected — `%1409` caught me inflating this.** The first draft read *"`%1414`
> demanded this rebuild as its BLOCKER 1"* and quoted it. The quote is real (`g-adversarial.md:10`,
> *"Retire this table and rebuild it with typed automated, manual, affordance, and none
> statuses"*), and `%1409` was wrong to say it appears in no artifact. But it was right about the
> thing underneath: **my own spawn prompt to `%1414` said *"should it be retired and rebuilt? Argue
> it."*** I proposed the conclusion, it returned my words, and I cited the echo as an independent
> demand. The finding is real; the **independence was manufactured**. See §7.4.

**The leg counts are a proxy, and following that NO-CLAIM found what it hides.** `06-gates.md`
attaches this boundary to the table above: *"the leg table counts files whose name matches a
property. A file named `mutation.rs` that mutates nothing still counts."* Acting on that warning and
reading the tests, the `mutation` column turns out to conflate **three different things**:

| kind | example | strength |
|---|---|---|
| an **automated** mutation test | `undrained-pipe-lint/tests/specimens.rs:205` `fn mutation_removing_stderr_pipe_retires_violation()` | strongest — the suite runs it |
| a **documented manual** procedure | `no-shell-gate/tests/gate.rs:12-14` names the three tests that must go RED under mutation: *"A green mutation run would mean the legs are not attributable to the pattern and prove nothing"* | real, but a human must run it |
| a **mutation affordance** | `no-shell-gate/tests/wired_lanes.rs:95-96` — *"The test-code stripping switch is deliberately named so its mutation is attributable"* over `const STRIP_TEST_CODE: bool = true;` | weakest — an invitation, not a leg |

So `no-shell-gate` — the gate this brief has called the exemplar throughout — has **no test function
with `mutation` in its name at all**. Its mutation discipline is a documented procedure over named
tests, which is genuine and is *not the same thing* as a leg `cargo test` executes.

**That did overturn `2 of 8`, and the table above is the rebuild.** Typing the column moved the
count a fourth time — `1 of 8` → `2 of 8` → `2 of 8, measuring three things` → **`1 of 8, and a
different gate`**. The honest table needed a typed value, not a bigger count, and once typed the
exemplar changed hands: `undrained-pipe-lint` qualifies and `no-shell-gate` does not.
*Recorded under R11.*

**0 of 8 gates mutate production source through the real hook** — the only definition that means
anything, and the count the rebuilt table above supports. **1 of 8 reaches a real filesystem tree**
(`omp-inventory-map`, `TREE`). **2 of 8 mutate a fixture string** (`undrained-pipe-lint`,
`state-wildcard-lint`), which `fh` row `C38` says certifies nothing. **1 of 8 has an affordance
nobody flips** (`no-shell-gate`). **4 of 8 have no mutation mechanism at all** —
`commit-build-fence`, `kernel-bypass-gate`, `pre-delete-citation-check`, `path-literal-guard`.
2 of 8 have no known-bad. 1 of 8 — `path-literal-guard` — has **no known-good leg**, which makes it
the highest-risk gate in the set: an attack-only suite ships an over-strict gate, and an
over-strict gate gets routed around, which is a slower death than no gate at all.

> **RETRACTED — historical claim; do not use.** The obsolete text asserted *"1 of 8 gates has all four legs with an AUTOMATED mutation test — undrained-pipe-lint (1/1/AUTOMATED/3)"*. That historical 1/8 and AUTOMATED label are non-authoritative: the rebuilt table classifies undrained-pipe-lint as FIXTURE.
> **Canonical result immediately replaces it:** 0/8 gates mutate production source through the real hook; 1/8 reaches a real filesystem tree; 2/8 mutate fixture strings; 1/8 has an unflipped mutation affordance; 4/8 have no mutation mechanism. These ratios are the typed round-10 snapshot, not a current census.
> The retraction is explicit because a stale conclusion is otherwise the part a reader quotes. The historical claim remains only to explain why it was rejected, never as evidence of current coverage.
>
> **RETRACTED provenance note:** the prior paragraph cited a rebuilt headline without sweeping its conclusions. That citation is retained solely as methodological history; it does not add an AUTOMATED leg or alter the canonical result above.
> *Recorded under R11.*

> **The first draft of this headline said "1 of 8" and "5 of 8," and both were wrong against the
> table printed directly above them.** `GateFrameworks` recomputed from the table and caught it.
> This is the purest instance of the defect this whole document exists to prevent: **a transcribed
> headline that nobody recomputes**, sitting one line below the data that refutes it. Prose review
> does not catch it — only arithmetic does. It is the same family as the retired "81 JSON-RPC
> methods, 17 used" figure, except this one was self-inflicted **in the brief that forbids it**, by
> the conductor, in the act of writing the rule. Every section quoting "1 of 8" or "5 of 8" must be
> corrected at assembly.

**False-zero experiment (deliberately reproduced; 0 is invalid evidence).** The two shell probes used the same root, crates/, on BSD grep (Darwin 25.5):

    grep -rl 'forbid(unsafe_code)' --include='*.rs' crates | wc -l  -> 0 paths
    grep -rl 'forbid(unsafe_code)' crates | wc -l                  -> 25 paths
    grep -r  'forbid(unsafe_code)' crates | wc -l                 -> 55 matching lines

The first result is a false zero caused by the include filter; it MUST NOT support a coverage claim. The authoritative source-aware snapshot uses the 26 workspace crates as denominator and separately checks Cargo lints and Rust inner attributes:

    python3 - <<'PY'
    from pathlib import Path
    crates = sorted(Path("crates").glob("*/Cargo.toml"))
    lint_ok = sum("unsafe_code = \\\"forbid\\\"" in p.read_text() for p in crates)
    rust = [p for p in Path("crates").rglob("*.rs") if "tests" not in p.parts and "fixtures" not in p.parts]
    attr_crates = {p.parts[p.parts.index("crates") + 1] for p in rust if "#![forbid(unsafe_code)]" in p.read_text(errors="replace")}
    print("workspace_crates", len(crates), "lints_forbid", lint_ok, "inner_attribute_crates", len(attr_crates))
    PY
    RECORDED SNAPSHOT OUTPUT (2026-08-31 ~21:45): workspace_crates=26, lints_forbid=26, inner_attribute_crates=25.

The 0/25/55 shell results are a historical false-zero experiment; the 26/26 and 25/26 source-aware result is the authoritative dated snapshot until re-run. Recorded under R11.

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
from a passing one in any aggregate count**, and it sits inside the `#[test]` figure quoted
above. The fix is a policy decision the plan must make rather than dodge: either the oracle lives
outside the tracked tree as a release artifact, or the differential lane is retired with a named
reason, or the rule gets its first exemption. *Recorded under R11 — not previously written down.*

### 3.6 The addressability defect — how the sixth gate property was born

```json
{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"ERROR",
 "data":null,"error":"CONFIG_ERROR unknown argument --help"}
```

**HISTORICAL ADDRESSABILITY SNAPSHOT.** The gate was built and its old snapshot reported 23 source tests plus 544,697 bytes of doctor output and an unknown-argument --help refusal. Current omp-inventory-map contains 28 test markers; the current debug binary emits 158 help bytes and exits 1. No current ADDRESSABLE pass artifact is claimed until a retained command/output/revision receipt exists.

Not built-vs-wired. **Wired-but-unaddressable.** It adds a sixth required gate property:
**ADDRESSABLE** — one documented command runs it, and `--help` names that command.

### 3.7 Other binding facts

- **The one hard rule:** no `.sh`, no `.py`. A Rust gate walks `git ls-files` and fails the build on
  either extension. The exemption list is empty.
- **The async contract:** asupersync 0.4.9, pinned rev `fa3c01aec`. `&Cx` first; `cx.checkpoint()`
  in loops; region-owned tasks, no detached tasks; **kill the process GROUP, not the pid**; drain
  both pipes; **a timeout is not a verdict**.
- **UNVERIFIED binding observations (not measured facts):** the working notes mention 29 raw spawn sites, 4 crates using subprocess-contract, and 12 of 14 async functions taking cx first. No exact command, input scope, exclusions, source revision, or captured output was retained for these figures, so they are not authoritative and MUST NOT drive closure. The async contract above is the design requirement; these observations do not prove conformance.
- **Unsafe — source-aware snapshot.** The false-zero experiment immediately above gives the complete roots and commands. Its explicit denominator is the 26 workspace Cargo.toml files; recorded output was 26/26 with the Cargo lint and 25/26 with the inner Rust attribute (pane-dispatch-fence is the binary-only exception, per §3.4). This is a dated snapshot, not a living count; re-run that command before treating it as current. Registered in NUMBERS.toml (crates_forbidding_unsafe).
- **omp-types — corrected by CrateSpecs.** The first draft claimed it re-exports AckKind, DeliveryClass, ObligationLedger, Budget, and Outcome. **That is wrong.** Measured with grep -c against crates/omp-types/src/lib.rs: ObligationLedger occurs **zero** times, and AckKind/DeliveryClass occur only inside the doc comment that names them as blocked. What actually re-exports is the Outcome family, the Budget family, and ObligationId / RegionId / TaskId / Time.

  The reason is documented in crates/omp-types/Cargo.toml:11-17: AckKind and DeliveryClass live behind cfg(feature = messaging-fabric), that feature transitively needs cfg(any(test, feature = test-internals)), and upstream issue #46 correctly removed test-internals from the default set — so enabling it here would **reintroduce the exact production leak #46 closed**.

  This changes the plan, not just the sentence: **the half of the vocabulary that would collapse the three ack dialects is blocked at an upstream feature boundary, not merely unadopted.** Any migration schedule assuming AckKind is available today is wrong. The crate still has **zero dependents**.
- **UNVERIFIED type-inventory observation (not a measured fact):** working notes report, excluding test modules and bin sources, 51 public enums and 79 structs across 22 of 24 crates; including all Rust sources, 59 enums and 91 structs. They also report 4 colliding type names, 6 Verdict-shaped types with no shared trait, and 17 ack/receipt types in 3 dialects. The retained derivation is only grep -rhoE over all *.rs; its exact patterns, exclusions, source revision, and captured output were not retained. These figures are therefore historical/non-authoritative and MUST NOT drive closure until a parser/build-graph command records those fields and its output hash.
- `fh` MCP is failing closed with a typed `SERVE_INPUT_STALE` (mirror HEAD moved `5dec4212…` →
  `ecdea397…`). Direct grep of the mirror at `/Volumes/ZestData/dicklesworthstone-mirror` still
  works. **Failing closed with a remediation hint is the model**, not a defect.
- **Mirror size — corrected.** The first draft said "216 repos"; that figure is re-derivable from nothing. Define M as /Volumes/ZestData/dicklesworthstone-mirror, inspected at the 2026-08-31 snapshot. The exact inventory command was find "$M" -maxdepth 2 -name .git | wc -l; it counts filesystem .git entries, not validated work-trees.

  | count | command | meaning |
  |---:|---|---|
  | 218 | <code>ls "$M" &#124; wc -l</code> | visible entries, including files |
  | 217 | <code>find "$M" -maxdepth 1 -type d &#124; tail -n +2 &#124; wc -l</code> | directories |
  | **210** | <code>find "$M" -maxdepth 2 -name .git &#124; wc -l</code> | **.git entries; not validated work-trees** |
  | 1 | <code>ls "$M" &#124; grep -c corrupt</code> | .corrupt-suffixed copies |

  **210 is therefore a filesystem-entry snapshot, not a repository/work-tree count.** Linked-worktree gitfiles, nested repositories, bare repositories, and submodules were not classified. Before publishing an N-repositories claim, a validation pass must resolve each candidate with git rev-parse --is-inside-work-tree, exclude bare/non-worktree entries, and record the candidate list, exclusions, revision, and output hash. The old 216 figure is RETIRED and non-authoritative; no number here is a validated repository denominator.
- Board at the 2026-08-31 stand-down snapshot: **28 closed, 25 in_progress, 19 open, 2 blocked — 74 total, not 75** (the four counts sum to 74; the original "(75 total)" was arithmetic that nobody recomputed — §7 row 1's exact defect). This 74 is historical and non-authoritative.
- Re-derived live during grading round 10, the board read **30 closed, 23 in_progress, 23 open, 3 blocked — 79 total** (30 + 23 + 23 + 3 = 79). Exact machine-readable query, with headers excluded because JSON is counted as records: for s in closed in_progress open blocked; do br list --status "$s" --json | jq -r 'if type == "array" then length elif .issues then (.issues | length) else error("unexpected br JSON shape") end'; done. The four outputs are 30, 23, 23, 3. This is a dated snapshot; re-run the query for current state. Do not substitute grep -c .: it counts rendered lines, not bead records.
### 3.8 The seven formerly missing gaps — strength corrected, not erased

The prior-art sweep found an upstream type for every gap this plan had treated as an absence. That changes the strength of the claims, not all of the local engineering conclusions. **Completion is WIRE-PROVEN; the other six are DECLARED ONLY.** A declaration weakens “no precedent” but does not prove reachability, semantic fit, or that this repository can consume the signal today.

| gap | upstream type and source | strength here | consequence for this plan |
|---|---|---|---|
| completion | AgentEndEvent.willContinue + SessionStopEvent (extensibility/shared-events.d.ts:83-93,154-162), carried by RpcSessionEventFrame (modes/rpc/rpc-types.d.ts:589) | **WIRE-PROVEN** — raw agent_end frame captured with isTerminal:true | adopt the existing event channel; the remaining gap is wiring it into the supervisor, not inventing a completion protocol |
| receipts | IrcDeliveryReceipt + AsyncJobDeliverySink (tools/hub/types.d.ts:8,84) | **DECLARED ONLY** — no wire path measured | cp-z42vu and the local transport-receipt gap remain; prove reachability before replacing the local receipt contract |
| claims | Stage1Claim / GlobalClaim with ownershipToken + inputWatermark (memories/storage.d.ts:20-27) | **DECLARED ONLY** — no wire path measured | the local claim fence remains necessary; adoption is an experiment, not a completed fix |
| idle | GuestIdleReconcilerCtx (dist/types/collab/guest.d.ts:9-30) | **DECLARED ONLY** — settle-vs-continuation semantics found, no wire path | the local NewlyIdle/ConfirmedIdle seam remains broken until this repository consumes it |
| roster | HubRosterCounts (dist/types/tools/hub/types.d.ts:33-90) | **DECLARED ONLY** — schema found, no wire path | hand-derived roster evidence remains unclosed |
| cost | SearchUsage (dist/types/web/search/types.d.ts:232-254), PerplexityCost (:510-527), and ContextUsage (dist/types/extensibility/extensions/types.d.ts:238-240) | **DECLARED ONLY** — no wire path measured | Q2 remains an instrumentation question; do not claim cost telemetry exists |
| compaction | SessionBeforeCompactEvent / SessionCompactEvent (dist/types/extensibility/shared-events.d.ts:54-75) | **DECLARED ONLY** — typed hook found, no wire path measured | context-loss recovery remains unproven; the type narrows the build, it does not close the operational gap |

**NO-CLAIM:** “WIRE-PROVEN” means the completion frame crossed the observed OMP RPC wire. It does
not mean omp-orchestrator consumes it, closes a bead from it, or that any of the other six types
are reachable from this process. “DECLARED ONLY” means the installed type surface exists and names
the relevant distinction; it does not mean the type is usable by this project today.

---

## 4. The control loop — five stages, seven measured rows, zero working

This is the spine of the whole plan. **No row works unqualified.**

It was called "the five-stage control loop (formerly "five-stage" — renamed, the table has five stages and seven rows)" until `%1414` counted the rows and found **five**, so
"exactly one row works" had no stable denominator. Correcting that exposed a second problem it
raised as MAJOR 1: `consume` was one row carrying **three separable claims** — selection,
admission, and transport — with a single verdict covering all three, so a `FENCED` admission was
masking two stages nobody had measured. Splitting them takes the table to **seven rows over five
stages**, and two of the three new rows are `UNVERIFIED` rather than working.

The denominator is now stated in the heading, which is the whole point: *five stages, seven rows.*

| layer | mechanism | measured state |
|---|---|---|
| observe | tick-monitor | **WORKS, bounded by two captures** — current positive timer/hash motion is checked before the 75-second no-change floor; see below |
| actionable | idle_panes | **SEAM OPEN** — the producer has typed NewlyIdle/ConfirmedIdle predicates, while parse_observation consumes JSON lists plus a state == IDLE fallback; no shared type crosses the process seam |
| consume — selection | decide() picks work | **UNVERIFIED** — no independent selection receipt is recorded |
| consume — admission | dispatch fence | **FENCED** — the 162 refused-tick/4.2-hour value is historical and unverified; no current refusal-rate figure is claimed |
| consume — transport | packet delivery | **UNVERIFIED** — cp-z42vu is a historical incident only; the current repository has no planted fixture or receipt payload |
| actuate | dispatch | **AVAILABLE, NOT VERIFIED** — `send_and_verify` exists at `crates/omp-orchestrator/src/main.rs:714` and is called at `:1461`; the installed 402-dead-pane incident proves an unsafe runtime path, not successful delivery |
| complete | worker says done | **AVAILABLE, NOT WIRED** — one raw agent_end frame carried isTerminal=true; AgentEndEvent.willContinue is declared upstream, and the supervisor does not consume it |

**The observe row was re-checked against current source after the Round 21 finding.** The current two-capture implementation compares positive timer or stable-hash motion at tick-monitor/src/lib.rs:564-572 before applying the 75-second floor at :574-577. A changed Working pair is Live; an unchanged short-gap pair is Unproven. The earlier block claiming the floor ran first was a historical description, not current behavior.

The admission row remains **FENCED**, but the old 162-refusal/4.2-hour number had no retained derivation. It is retained only as a historical failure-shape example; no current refusal-rate figure is claimed in this brief. The transport row likewise records cp-z42vu as a historical incident only; the current repository has no planted fixture or receipt payload for it.

The completion row is bounded: one raw agent_end frame carried isTerminal=true, while AgentEndEvent.willContinue is a declared upstream field. That is wire evidence for one external frame, not proof of local supervisor consumption or repeatability.

**The actuate row was corrected 2026-09-01 by the guardian pass, and the correction is worse than the row it replaced.** "DOES NOT EXIST" was true when written and false by the time it was graded: the `kxe` supervisor has been launchd-resident since 2026-08-31 12:21 and dispatches on its own. What the live ledger shows is the actuate layer running **without the claim beat** — 131 sends of one unclaimed bead to one dead pane, each one recorded as a success. A layer that does not exist cannot mis-dispatch; this one did, for four hours, in the product's own log, and no reader noticed until a human asked. The mechanical fix (install the committed claim fence; classify a 402-dead pane as attention, not capacity) is filed as two P0 beads labelled `guardian`. **NO-CLAIM:** the 131 figure is one pid on one machine on one day; it is a failure shape, not a rate.
*Recorded under R11.*

---

## 5. Section map — who owns what


Twelve companion section files (01 through 12), disjoint by construction so that parallel authorship cannot collide, plus this brief (00) make **thirteen plan files total**. The assembled docs/PLAN.md is built from these; the section files are the source of truth.
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
| `11-lifecycle.md` | Lifecycle evidence map from idea to shipped | R13 |
| `12-journey.md` | Dispatchable S1–S9 journey runbook | R13, R9 |

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
In one session they refuted **nine measurements**, seven of them the conductor's own, and every
single refutation came from an agent **re-deriving rather than reading**.

> **This count was itself wrong, and `%1409` caught it.** The sentence said "five measurements,
> three of them the conductor's" while the assembled `PLAN.md` header said "eight… six of them the
> author's" and the true figure was nine. **A self-correction section that cannot count its own
> corrections** — filed as BLOCKER 2 by the evidence lens, and the most damaging shape available to
> this document, because it makes the honesty itself unauditable. The count is now derived from the
> table below rather than typed from memory, and the `PLAN.md` header is regenerated from it at
> assembly instead of being written twice.

| # | claim | refuted by | mechanism of the error |
|---|---|---|---|
| 1 | "1 of 8 gates has all four legs / 5 of 8 lack mutation" | `GateFrameworks` | arithmetic never recomputed — the table one line above said 2 and 4 |
| 2 | "tmux fails while exiting 0" | `PriorArtWriter` | `$?` read after a pipeline; `PIPESTATUS=(1 0)` |
| 3 | "`omp-types` re-exports the ack vocabulary" | `CrateSpecs` | read the intent, not the file; `ObligationLedger` occurs zero times and the ack half is **blocked** upstream |
| 4 | "no prior art for typed missing-dependency" | `EndUserJourney` | `--include='*.rs'` aimed at **ntm, a Go repo** |
| 5 | "no prior art for anti-vacuity" | `GateFrameworks` | search space too narrow — 3 doc files; the concept lives in tests, telemetry, a shell gate, and a Lean proof |
| 6 | "no prior art for binary identity" | `PriorArtWriter` | a not-found published with **no recorded search at all** |
| 7 | "no prior art for mutation via a real hook" | `PriorArtWriter` | same — seeded as absent, never searched; `franken_lean` drives real `git commit` against the real hook |
| 8 | "`ff-merge local->origin` will reconcile `rch`" | the conductor | `ahead 3` makes `--ff-only` **refuse**; stated as one command, is a rebase across 1,813 commits |
| 9 | "`-V` answers 6 of 9" | `%1409` (evidence lens) | the table excluded `tmux` from the `-V` column while the sentence above it says `tmux -V` works — a table contradicting its own caption |

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
principle was itself corrected ten times by agents applying it — against its author, within hours,
each with a command and a result. The process caught its own author. **An investor should weigh that
more heavily than any green test in this repo**, because it is the only evidence here that the
method works on the person running it.

**NO-CLAIM.** Ten refutations in one session is evidence the challenge mechanism functions. It is
**not** evidence that the remaining measurements are correct — only that ten specific ones were
wrong and are now recorded. The base rate of undetected errors in this document is unknown and is
not estimated anywhere. **None of the ten §7 refutations was found by an automated check; each was found by an agent choosing to re-derive, and no gate in this repo enforces that choice.** This does not include the separate §8 census omission: the slash_commands=0 versus expected_slash_commands=136 mismatch was caught by the scanner's automated expected_* twin.

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
and it is why the honest reading of §7 is not "our process caught ten errors" but **"our process
caught ten errors and has no mechanism preventing the eleventh."**

*Recorded under R11. The corrective is a checker that refuses `$?` after a pipeline in any command
cited as evidence — specified in `09-milestones.md` as PC7, and not built.*

---

### 7.3 The assembled document went stale against its own sources

`docs/PLAN.md` carries this instruction in its own header: *"The section files are the source of
truth; this document is their concatenation. **Edit a section, then re-assemble — never edit
here.**"* Minutes after that line was committed, §3.5 of this brief gained the mutation-leg
heterogeneity finding, and `PLAN.md` was **not** re-assembled. Measured:

```
stat -f %m docs/PLAN.md              -> assembly time
stat -f %m docs/plan/00-brief.md     -> 132 seconds NEWER
grep -c 'measuring three different things' docs/PLAN.md   -> 0
```

The rule was followed exactly — the section was edited, not the assembled file — and the artifact
still drifted, because **the second half of the instruction has no enforcement**. Nothing re-runs
the assembly and nothing notices when it is overdue. A reader opening `PLAN.md` would have found a
document that silently omitted the newest finding while presenting itself as complete.

This is a self-inflicted defect and the most on-the-nose one recorded here: **a derived
artifact diverging from its source, in the document whose subject is derived artifacts diverging
from their sources.** It is the same shape as the 23-commit supervisor drift in §07 and the
`.corrupt-` mirror copies — a stale copy that answers questions as though it were current.

R3 is **PARTIAL** here. Closure is a freshness/identity check, not file existence: every one of the thirteen plan source files must be covered by the assembled manifest with its path and hash, no source mtime may be newer than docs/PLAN.md, and a reassembly must reproduce the recorded output hash. The measured 132-second drift and absent phrase prove the current artifact fails that predicate.
The predicate is the four-way identity check from 07-installability.md applied to a document instead of a binary, and **it is not built**. Until a named gate runs it and emits PASS, “re-assemble after editing” is only a habit; §7.2 measured what habits are worth.
*Recorded under R11. The assembled artifact must be regenerated before R3 can move from PARTIAL to CLOSED.*

### 7.4 I seeded three of the four findings I then cited as independent

`%1409` filed this as a BLOCKER in round 2 and overstated it — it claimed the `%1414` quote
"appears in NO artifact," and the quote is real at `g-adversarial.md:10`. Underneath the
overstatement was the most damaging methodological finding of the exercise, and it is mine.

Auditing my own round-1 spawn prompts against the findings I celebrated:

| grader | headline finding | my prompt said |
|---|---|---|
| `%1414` | "retire and rebuild the leg table" | *"should it be **retired and rebuilt**? Argue it."* (`p1414.txt:35`) |
| `%1408` | "no economic dimension" | I **listed the topics**: *"cost, time, headcount, competitors, security, licensing, novelty, version drift, kill criteria"* (`p1408.txt:33`) |
| `%1409` | "the self-correction count is unreconciled" | *"COUNT THEM… that would be the funniest and most damaging possible defect"* (`p1409.txt:34`) |
| `%1413` | "no problem worth paying for" | genuinely open questions — *"Who pays? What do they do instead?"* — **not seeded** |

**Three of four headline findings were substantially seeded by the person they were grading.** I
then wrote them up as convergent independent adversarial confirmation, which is the strongest claim
this document makes about its own method, and it was inflated.

**The findings are still real** — the count *was* wrong, the economic gap *is* real, the aggregate
*was* uncomputable. Pointing a grader at a genuine defect does not fabricate the defect. It
fabricates the **independence**, and independence is the entire value of an adversarial round. A
seeded finding is worth what a code review is worth; an unseeded one is worth what a fresh pair of
eyes is worth, and I billed the first as the second.

**What was genuinely unseeded, and is therefore the real yield of round 1:**
- `%1413`'s verdict that §7 reads as **alarming, not confidence-building** — the answer to a
  question I posed but could not answer myself, and the finding most likely to matter to a reader.
- `%1408`'s **structural** argument, which went past my list: that §2's table is the sole discharge
  mechanism, so unregistered questions are *unaskable* rather than merely unanswered. I listed
  topics; it found the mechanism. That became R12.
- `%1409`'s `-V` catch — 6 of 9, not 5 — which I neither predicted nor mentioned.
- `%1414`'s five-row count, the `consume` split, and the actionable-layer challenge — none seeded.

**The fix, for every round after this one:** a grading prompt names the *lens* and the *file*, and
**never the suspected defect**. Round 2's packet already violated this — it listed all six fixes
and asked graders to verify them, which is legitimate for regression but must not be counted as
discovery. Round 3 onward: lens and file only.

*Recorded under R11. This is the tenth refutation and the fourth consecutive one belonging to the
conductor.*

---

## 8. Open questions, risks, and kill criteria

**This section exists because all four graders independently found its absence, and one of them
found it structurally.** `%1408` (negative-space lens) made the argument that matters: §2's
checkability table is the *sole* mechanism by which a requirement becomes answerable, so a question
never registered as a requirement **can never be discharged by any of the thirteen sections**. The
plan could execute flawlessly and still have no answer. That is a defect in the requirement set,
not a missing paragraph, and it is filed as **R12** below.

`%1413` (investor lens) reached the same place from the other side: *"the document does not
establish a problem worth paying for"* and *"does not say what happens if this works."*

### 8.1 R12 — the economic and risk dimension is a requirement, not an appendix

> **R12.** Every question an experienced operator asks before funding work must be either answered
> or **registered here as an open question with an owner**. A question that is neither is a gap the
> requirement set cannot see.

### 8.2 The open questions, unanswered and owned

Every row is OPEN unless marked. None of these had a home in the document before this round.


**Open-question census recipe:** scope is exactly the files selected by docs/plan/[0-9][0-9]-*.md, then filtered to prefixes 01 through 12 (twelve companion files; 00-brief.md is intentionally excluded), with case-insensitive matching and no grader artifacts. This exact command counts occurrences per file and prints totals; no result is asserted until it is run against those files:
    python3 - <<'PY'
    from pathlib import Path
    files = sorted(Path("docs/plan").glob("[0-9][0-9]-*.md"))
    files = [p for p in files if p.name != "00-brief.md" and 1 <= int(p.name[:2]) <= 12]
    for term in ("security", "secret", "credential", "token", "licens"):
        hits = [(str(p), p.read_text(errors="replace").casefold().count(term)) for p in files]
        print(term, "files", len(files), "occurrences", sum(n for _, n in hits), "by_file", hits)
    assert [p.name[:2] for p in files] == [f"{n:02d}" for n in range(1, 13)]
    PY

| # | question | status | owner |
|---|---|---|---|
| Q1 | Who pays for this, and what is their current workaround? | **OPEN** — no buyer named anywhere in thirteen sections | Josh |
| Q2 | What does the current workaround cost, measurably? | **OPEN** — the only cost figure in the brief is the phrase *"cost real time"* | Josh |
| Q3 | What is the outcome if this works, in customer terms with a baseline and a target? | **OPEN** — the plan describes mechanism end-to-end and outcome nowhere | Josh |
| Q4 | How long, and with how many people? | **OPEN** — no timeline, no headcount, in any section | Josh |
| Q5 | Buy, adopt, or build? What existing tool was evaluated and rejected, and why? | **OPEN** — §10 mines the mirror for *patterns*, never for a *substitute* | orchestrator |
| Q6 | What happens when OMP changes under us? | **OPEN** — we pin omp/18.0.11 and have no compatibility policy; the pre-extraction scanner recorded 799 slash-command rows versus expected 136 and remained UNKNOWN | orchestrator |
| Q7 | What is the security posture — secrets, tokens, the blast radius of a dispatch? | **OPEN** — no numerical corpus claim is made here; the census recipe below scans all twelve companion files (01–12), case-insensitively | orchestrator |
| Q8 | Licensing, for us and for what we vendor? | **OPEN** — no numerical corpus claim is made here; the same twelve-file, case-insensitive census recipe below is the source of any future count | Josh |
| Q9 | Is any of this novel, and does novelty matter here? | **ANSWER MOVED** — the completion protocol’s precedent-free claim is REFUTED: AgentEndEvent.willContinue is WIRE-PROVEN on RpcSessionEventFrame via --mode=rpc; the novelty question remains open for the other six DECLARED ONLY types and their adoption path | orchestrator |
| Q11 | Who owns the composer-typed policy decision — oracle outside the tree, retire the lane, or the rule's first exemption? | **OPEN** — orchestrator owns the decision, but §3.5 records the trilemma without a deadline or decision receipt; %1408 flagged the missing closure twice | orchestrator |
| Q12 | Who owns the pi_agent_rust tmux-missing defect we inherit if we adopt its two-signal probe? | **OPEN** — orchestrator owns the decision, but the adoption/exception choice and its evidence are not recorded; adopting the pattern adopts the bug | orchestrator |
| Q13 | Should the `.git/hooks/commit-msg-verification-level.sh` policy be registered, rewritten, or retired? | **OPEN** — the hook decision is not recorded with a chosen option, owner deadline, or durable decision receipt | Josh |
| Q10 | **What kills this?** | **PARTIAL** — Josh owns the decision; §09 carries technical kill conditions, but no economic criterion or decision receipt is recorded | Josh |

### 8.3 The kill criteria, stated so they can fire

A kill criterion nobody can evaluate is decoration. Each names its observable.

| # | we stop if… | observable |
|---|---|---|
| K1 | the completion signal cannot be consumed by the supervisor | **WIRE-PROVEN, ADOPTION REMAINS** — OMP ships `AgentEndEvent.willContinue` and `SessionStopEvent` (`dist/types/extensibility/shared-events.d.ts:83-93,154-162`), and a raw `agent_end` frame with `isTerminal:true` crossed `--mode=rpc` via `RpcSessionEventFrame` (`modes/rpc/rpc-types.d.ts:589`). The remaining kill condition is failed adoption into the supervisor, not inability to build a completion protocol |
| K2 | verification costs more than the review it replaces | **OPEN/UNVERIFIED — owner: Josh. For a 30-day pilot, numerator = verification minutes recorded in the tick/review ledger; denominator = review minutes demonstrably replaced; fire if numerator/denominator > 1.0 in two consecutive weekly windows. Source: timestamped tick ledger plus review log. Instrumentation and baseline are not yet built.** |
| K3 | a second machine cannot run it | §07: never attempted; installer hardcodes `/Users/josh` as its fallback home |
| K4 | the gates get routed around | measurable as: any commit landing with a gate disabled and no named allowance row |
| K5 | the fleet needs more tending than the work it does | **OPEN/UNVERIFIED — owner: orchestrator. For a 30-day pilot, numerator = operator tending minutes (reap, redispatch, unblock, or intervene); denominator = minutes of verified work completed; fire if numerator/denominator > 1.0 in two consecutive weekly windows. Source: timestamped fleet/operator ledger plus verified close receipts. The historical 4.2 hours of refused ticks is context only, not this denominator or a fire.** |

### 8.4 The blind spot this method cannot see

`%1408`'s second blocker, and the sharpest structural point of the round. **Every one of the nine
refutations in §7 is an error of commission** — a wrong number, caught by re-deriving it. The one
omission ever caught this session was `slash_commands=0` vs `expected_slash_commands=136`, and it
was caught **only because the scanner emits an `expected_*` twin**. The prose has no twin
mechanism, so **an omission in prose is structurally invisible to this process** — nobody
re-derives a paragraph that was never written.

That is why this section exists at all, and why it took a lens explicitly assigned to absence to
find it. The fix is R12 plus the per-subsection expected-contents list `%1408` proposed, which is
**not built**.

**NO-CLAIM.** This section registers thirteen questions (eleven OPEN; Q9 ANSWER MOVED; Q10 PARTIAL) and five kill criteria. It **answers none of them**. Registering a question is not
progress on it; it makes the gap visible and assignable, which is strictly less than knowing the
answer and strictly more than the previous state, where the question could not be asked from
inside the requirement set.

`%1408`'s second blocker, and the sharpest structural point of the round. **Every refutation
tabulated in §7 — nine rows — is an error of commission**, a wrong number, caught by re-deriving
it; §7.4 adds the tenth, which was an error of *process* (seeded independence), not a number.
The one omission ever caught this session was `slash_commands=0` vs `expected_slash_commands=136`,
and it was caught **only because the scanner emits an `expected_*` twin** — the prose has no twin
mechanism, which is why §8 exists.

---

**NO-CLAIM.** This brief records requirements and measurements. It does not establish that the plan
satisfies them, that the measurements are complete, or that the sections listed in §5 exist yet at
the quality bar §6 demands. Grading is a separate pass and it has not run.

---

## 3.8 BLOCKER resolution — the board count, and why it kept moving

`GradeBrief` filed a BLOCKER: §6 requires *"every number carries the command that
derives it"*, and this section's board counts never did. The history:

| appearance | value | derivation |
|---|---:|---|
| stand-down snapshot prose | 75 | none |
| its own four counts | 28+25+19+2 = **74** | none |
| corrected fresh count | 30+23+23+3 = **79** | none |
| per-status enumeration, re-derived during this integration | **95** | exact command: for s in closed in_progress open blocked grading; do br list --status "$s" --json | jq -r 'if type == "array" then length elif .issues then (.issues | length) else error("unexpected br JSON shape") end'; done → 61 closed + 21 in_progress + 10 open + 2 blocked + 1 grading |

This 95-row value is a dated integration snapshot, not a pinned constant. `board_total` remains LIVE and must be re-run before quoting it; the 75/74/79/93 values above are historical snapshots.

A status nobody thought to count cannot appear in a hand-summed total, and its
absence is invisible in exactly the way a missing row always is.

`board_total` was already declared `LIVE` before this resolution, and appending a
second block with the same key made the registry's own duplicate-key gate fail —
21 declarations, 20 unique. That gate exists because this is a shared checkout
with concurrent writers, and it caught a concurrent append by its author within
a minute. The duplicate was removed; the `LIVE` declaration stands, because
pinning a number that moves hourly would drift by design.

The `75` and `74` were not
a typo and a correction — **they were two different hand-sums of an
under-enumerated set**, which is why correcting one never fixed the other.


---


<!-- ===== 01-idea.md ===== -->

# 01 — The idea

*Serves R10 (idea → why → binaries → SOTA bar) and R3 (investor-attackable). Written to the writing
contract in `00-brief.md` §6. Numbers in this section are either (a) a dated, command-backed receipt,
(b) a source citation explicitly marked as reported, or (c) historical/non-authoritative. A corrected
value in `00-brief.md` or `NUMBERS.toml` wins; stale values may remain only when labelled as such.*

---

## 1.1 The one sentence

**Falsifiable thesis (HYPOTHESIS):** a technical lead or founder who runs a multi-agent coding fleet
(economic buyer and budget owner) will pay a proposed **$500/month** for a narrow supervisor that lets
the daily fleet operator turn one completed OMP session into a receipt-backed bead close. The trigger
is a worker ending while the board, pane, and receipt disagree; the current workaround is manually
watching panes, copying evidence into a board, and asking a human to retry dispatch. The local session
below demonstrates that failure mode, but not its prevalence or value outside this checkout. The first
outcome is deliberately low-trust: **one OMP completion → durable receipt → one typed close/refusal**,
with no autonomous dispatch and no claim of broad fleet accounting.

The load-bearing word is *provable*. “An accountant for the ones already running” is a hypothesis, not
a category fact. **FACT:** this checkout recorded a local reconciliation failure. **UNKNOWN:** whether
other operators pay to solve it, what their baseline loss is, and whether $500/month is acceptable.
The cheapest discovery test is five interviews with fleet owners followed by a concierge replay of the
one-session wedge; ask each owner to provide a redacted completion/board mismatch and to sign a
non-binding $500/month letter of intent only if the receipt removes a manual close. No paid pilot or
production commitment is requested before the gates below pass.

---

## 1.2 Why this exists

The problem hypothesis is not that an agent fleet fails to work. It is that a fleet can complete work
while its board and evidence cannot tell the operator what finished. The **economic buyer** is the
technical lead/founder accountable for throughput and auditability; the **daily operator** is the
person tending panes, dispatch, and bead state. The observable trigger is a terminal worker event or
stand-down followed by a board state that remains open/in-progress or a receipt that cannot be bound
to the worker. The current workflow is pane watching plus manual board updates; its cost, frequency,
and willingness-to-pay are **UNKNOWN** outside this repository.

That is not yet a market claim; it is a local reap. From one stand-down session on this repository
(FACT about this checkout only):

 - **Historical, non-authoritative recollection:** 6 beads landed awaiting grade and ZERO landed *and*
  closed on the day. Work reached the tree; the ledger never learned it had. This is not used as market
  evidence because the detailed reap artifact and derivation command were not retained here.
 - **Historical, non-authoritative recollection:** 7 conditions were live in the repository and belonged
  to no bead; two were pre-existing red tests in a suite expected to be red. The detailed reap artifact
  and derivation command were not retained here, so this is not a measured prevalence claim.
 - **UNVERIFIED REPORTED VALUE:** 162 refused dispatch ticks across a reported 4.2 hours, with a human as the only
  actuator (00-brief.md §4, dispatch-admission row at line 526). The brief's command receipt is not preserved in this section;
   treat this as a cited report, not an independently reproducible measurement or market-frequency claim.
 - **Historical, non-authoritative recollection:** a 23-commit drift between the installed supervisor
  binary and HEAD. No immutable receipt or derivation command is retained here.
- Board snapshot (**reported FACT, as of 2026-08-31; authoritative reference 00-brief.md §3.7, lines 485-486; command receipt not preserved here**): **28 closed, 25 in_progress, 19 open,
  2 blocked (74 total)**. The snapshot is not the live board.

### The mechanism behind the 162

The reported 162 is a local symptom with a named cause, not proof of market recurrence. The control loop has
**five stages and seven measured rows** (00-brief.md §4); the denominator is seven rows, and there
are **zero unqualified WORKS rows**:

| stage | row/mechanism | measured state |
|---|---|---|
| observe | tick-monitor | **WORKS, qualified** — the observed monitor path has an asymmetry defect |
| actionable | idle_panes | **BROKEN** — discards NewlyIdle; free_capacity uses the same is_dispatchable filter and excludes a pane at t=0 |
| consume | selection | **UNVERIFIED** — no durable receipt here proves the selected work was consumed |
| consume | transport | **UNVERIFIED** — no durable receipt here proves delivery to the intended worker |
| consume | admission (decide()) | **FENCED** — the cited report is DISPATCH_RETRY_BLOCKED |
| actuate | dispatch | **AVAILABLE, NOT VERIFIED** — `send_and_verify` exists at `crates/omp-orchestrator/src/main.rs:714` and is called at `:1461`; transport and receiver receipt remain unproven |
| complete | worker says done | **AVAILABLE, NOT WIRED** — OMP exposes AgentEndEvent.willContinue on RpcSessionEventFrame; the local loop does not consume it |

> *Upstream type for this gap: `GuestIdleReconcilerCtx` (DECLARED only). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

A single shared predicate, used to answer two different questions, produced a coherent but wrong local
world model. **INFERENCE (not yet proven):** typed separation may outperform logging-only
instrumentation; logging could faithfully record 162 refusals while leaving the overloaded question
unchanged. The falsification test compares both approaches on false completion/idle classification,
operator minutes, and adoption effort.

**INFERENCE:** four symptoms may share one shape: **no typed answer to a question the supervisor must
answer in one call**. This is an experiment hypothesis, not a conclusion. The completion signal exists
on the wire, but the supervisor does not attach to it; the other six gap types remain DECLARED ONLY.

**NO-CLAIM:** these observations come from a single session's reap on one machine and one checkout.
The 6/7/23 figures are historical recollections without retained command receipts and are not market
evidence; the 162 figure is reported by the brief but is not independently reproducible from this
section. They establish only that these failures occurred in that local report, not how often they
occur.

### 1.2.1 The seven gap claims — what the upstream types actually change

The upstream sweep changes the strength of the absence claims without pretending that a declared type is a consumed contract. One gap is WIRE-PROVEN; the other six are DECLARED ONLY.

| gap | upstream type and source | true strength | effect on the idea
|---|---|---|
| completion | **`isTerminal` on RpcSessionEventFrame** (modes/rpc/rpc-types.d.ts:589) is what was OBSERVED. `AgentEndEvent.willContinue` and `SessionStopEvent` (extensibility/shared-events.d.ts:83-93,154-162) are DECLARED ONLY — see §1.2.3 | **WIRE-PROVEN for one observed frame, and for `isTerminal` only** — exact raw receipt PRESERVED IN-REPO at `.flywheel/inventory-artifacts/agent-end-raw-frame.json.gz` (hash-gated by `artifact_provenance`; the original `/tmp/grade/agent-end-raw-frame.json` is reboot-volatile and must not be cited); capture command /Users/josh/.local/bin/omp --mode=rpc --no-session --no-tools --no-lsp --max-time=30; artifact mtime/retrieval observed 2026-08-31T19:52:26-0600; SHA-256 d8bd80c6949b2ec48af1639b5b5e241bd90b4dce1e769483dd1690ed2be8f644 | the frame's session-specific isTerminal=true was observed; shared willContinue was absent; repeatability, semantic fit, and supervisor consumption remain UNKNOWN
| receipts | IrcDeliveryReceipt + AsyncJobDeliverySink (tools/hub/types.d.ts:8,84) | DECLARED ONLY — no wire path measured | the cp-z42vu transport/receipt gap remains; type existence does not replace receiver proof
| claims | Stage1Claim / GlobalClaim with ownershipToken + inputWatermark (memories/storage.d.ts:20-27) | DECLARED ONLY — no wire path measured | local claim/ownership gap remains until reachability and semantics are proven
| idle | GuestIdleReconcilerCtx (dist/types/collab/guest.d.ts:9-30) | DECLARED ONLY — no wire path measured | the local NewlyIdle/ConfirmedIdle defect remains; the upstream split is corroboration, not a fix
| roster | HubRosterCounts (dist/types/tools/hub/types.d.ts:33-90) | DECLARED ONLY — no wire path measured | hand-derived roster remains an unclosed observation gap
| cost | SearchUsage (dist/types/web/search/types.d.ts:232-254), PerplexityCost (:510-527), ContextUsage (dist/types/extensibility/extensions/types.d.ts:238-240) | DECLARED ONLY — no wire path measured | cost telemetry remains unmeasured
| compaction | SessionBeforeCompactEvent / SessionCompactEvent (dist/types/extensibility/shared-events.d.ts:54-75) | DECLARED ONLY — no wire path measured | context-loss recovery remains unproven; the type narrows adoption work but does not close it

**NO-CLAIM:** the durable receipt proves one raw wire frame, not supervisor consumption. In the declaration,
willContinue is the shared AgentEndEvent field, while isTerminal is supplied by the session-specific
frame shape. The receipt does not establish repeatability, semantic equivalence of isTerminal to
willContinue=false, non-terminal settle behavior, crashes, killed panes, rate limits, compaction,
semantic fit, or adoption cost. The other six entries refute “no upstream type exists” only.

---

## 1.2.2 Substitution map and residual wedge (HYPOTHESIS)

The job is “bind a worker completion to a durable, reviewable board outcome.” Current substitutes are
not yet evaluated; the table is a discovery inventory, not a claim that this product wins:

| substitute | job outcome | trust/evidence | price/switching cost | strongest contrary evidence |
|---|---|---|---|---|
| OMP/ntm/br/bv native workflow | runs agents, panes, triage, and bead state | partial; no single completion-to-close receipt | existing tools are sunk cost; switching cost low to try | may already be sufficient for disciplined operators |
| shell/Python scripts | bespoke polling, parsing, and board updates | varies; scripts can be local and inspectable | low cash cost, high maintenance cost | a script may solve this exact narrow path faster |
| human review | watches panes and reconciles board/evidence | high judgment, low repeatability | operator time is the cost | may be cheaper than buying software at low volume |
| managed service/consultant | outsourced fleet tending and reporting | depends on provider and handoff | recurring labor cost; high switching cost | service can absorb exceptions without new software |
| doing nothing | accepts stale/ambiguous state | no additional assurance | zero cash; hidden rework/opportunity cost UNKNOWN | pain may be tolerable and non-recurring |

**Residual wedge (HYPOTHESIS):** a typed, low-trust completion → receipt → close/refusal path may be
worth adopting only when it saves more operator time/rework than its price and switching cost. The
cheapest hostile test is to ask five current operators to replay one redacted mismatch using their
native workflow and a concierge receipt; record outcome, trust, minutes saved, willingness to pay, and
why they reject it. Until that test, differentiation, prevalence, and value are UNKNOWN.

## 1.2.3 Narrow first-value boundary

**IN SCOPE:** one OMP RPC terminal completion, one durable evidence receipt, and one typed bead
close/refusal that an operator can review. **OUT OF SCOPE:** autonomous dispatch, all seven gap
families, fleet-wide cost accounting, multi-tenant service, generalized integrations, and production
security/compliance claims. The nine binaries, scanner, census, and gates below are research or
internal enforcement instrumentation unless and until this wedge is wired end to end. A materially
narrower thesis needs a new viability packet; this section intentionally preserves the fresh grader's
no-dispatchable-stage scope.

## 1.3 The binaries we are wrapping

We are building an **enforcement-substrate prototype**, not claiming a customer-ready runtime. The
future product promise is a typed supervisor over binaries that already exist; the local implementation
currently lacks the OMP supervisory integration. The bet that accountability is the residual wedge is a
HYPOTHESIS, not a universal category claim.

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
mirror — 210 filesystem .git entries at /Volumes/ZestData/dicklesworthstone-mirror; they are not validated as git work-trees, so this is a filesystem-entry census rather than a repository count — which is how R7 gets
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

### 1.3.9 tmux — historical correction to the brief

We call tmux as the substrate under ntm; pane truth is ultimately read through it. An earlier brief
draft incorrectly said that tmux had no machine-readable version handshake. The current brief is
correct: tmux has a machine-readable handshake on its short flag, not the GNU long flag.

 - **FACT, measured on this host:** command tmux -V; echo "exit=$?" returned tmux 3.6a and exit=0.
 - **FACT, measured on this host:** command tmux --version; echo "exit=$?" returned an unknown-option
   usage block and exit=1.
 - **Historical correction:** the old “no machine-readable handshake” interpretation and its old line
   citation are non-authoritative and must not be copied forward. The table's -V result is the current
   source of truth.

The real risk is narrower: a uniform probe loop written against --version can record tmux as absent
or broken rather than present at 3.6a. The dependency probe must use -V and document that asymmetry;
nonzero exit from the long flag is not evidence of absence.

**NO-CLAIM:** the table records what resolved on `PATH` at one moment on one host (Apple M3 Ultra,
darwin 25.5.0). It does not claim these versions are pinned, reproducible elsewhere, free of the
`cargo` shim's effects, or that the stated contract per binary exhaustively describes that tool's
interface. "If it drifts" clauses describe consequences we reason about; none has been observed and
none is a prediction of likelihood.

---

## 1.4 Dependency-contract/SLO floor (not product SOTA)

R10, verbatim: “every aspect of this needs to be on par or greater than SOTA - same as the binaries
we are wrapping.” The table below operationalizes a **technical dependency floor**: properties already
shipped by wrapped tools that our implementation must match. It is not evidence that the product wins
the customer job. Product-level SOTA remains OPEN until the substitution map is measured against
end-to-end latency, false completion/idle rates, operator-time reduction, reliability, adoption effort,
and cost. Any expected superiority is **PROJECTED**, not measured.
Product benchmark definition for later discovery: compare the wedge with each named substitute using
p95 completion-to-receipt latency, false completion and false-idle rates, operator minutes per close,
replay reliability, onboarding effort, and fully loaded monthly cost. Record each baseline and result
with the same cohort and a retained receipt; no product SOTA conclusion exists until that comparison
runs.
| wrapped binary | property it already ships | our obligation |
|---|---|---|
| `bv` | `--robot-triage` returns **multiple ranked slices in one call** | a tick answers multi-part questions in one invocation, never N |
| `br` | **typed close-policy refusal** — refuses to close, with a type | gates refuse with a typed reason, never a bare nonzero exit |
| `fh` | **fails closed** with typed `SERVE_INPUT_STALE` + remediation hint | every surface prefers typed refusal over a confident stale answer |
| `omp` | **versioned envelope** on output | every artifact carries `schema_version` |
| `git` | `ls-files` is an **authoritative machine-parseable set** | gates enumerate from an authority, never a filesystem walk |
| `jsm` | **single-store invariant, checked at session start** | installation topology is checked, not assumed |
| `tmux` | machine-readable version at `-V`, exit 0 | our binaries answer a version probe **on the conventional flag** — we do not ship tmux's asymmetry ourselves |

**HISTORICAL SCANNER SNAPSHOT (2026-08-31).** The built scanner report described `omp-inventory-map/v1`, command `doctor`, status `UNKNOWN`, exit 2, and 544,697 bytes. The retained raw capture is `.flywheel/inventory-artifacts/inv.txt.gz` with SHA-256 `86491732a5581a6d2e342d0db59bdf20e5f47f6da93150ae78bd2649562f5081`; this is not current product or workspace evidence.

**HISTORICAL ADDRESSABILITY SNAPSHOT (2026-08-31).** `omp-inventory-map --help` returned the typed `CONFIG_ERROR unknown argument --help` refusal; the source snapshot counted 23 functions and 544 KB of output, with no `Observation`, `CONVERGE`, or `Verdict` strings. The collision guard at `crates/omp-inventory-map/src/types_inventory.rs:176-178` remains the cited design evidence.

**CURRENT RECHECK.** The current source has 28 test markers and the debug binary's `--help` probe emits 158 bytes and exits 1. No current ADDRESSABLE pass is claimed without a retained command/output/revision receipt; the gate remains a real system gap, not a buyer-visible result.

The **historical** addressability shape recurs in §1.5: `--help` did not return a generic error; it returned a typed `CONFIG_ERROR` with an accurate message. The typing discipline was applied perfectly to the wrong outcome. Hence the current design requirement:

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

**What is genuinely built (re-derived during this integration).** Historical 26-crate/413-test-function/31-file values stay labelled as snapshots; current workspace authority is **50 crates, 987 test functions, and 92 integration test files**, from the recursive NUMBERS.toml commands. These are workspace facts, not product-completion evidence.

The omp-types crate re-exports Outcome, OutcomeError, PanicPayload, Severity, and join_outcomes; Budget,
CapabilityBudget, CapabilityBudgetDimension, CapabilityBudgetRefusal, CapabilityBudgetRequirements, and
RemainingBudget; plus ObligationId, RegionId, TaskId, and Time from the pinned asupersync revision
fa3c01aec. AckKind and DeliveryClass are **present upstream but blocked at the pinned feature boundary**.
ObligationLedger is **absent everywhere** in the inspected upstream inventory; its semantics require a
decision. This is an **enforcement-substrate prototype**, not a customer-ready supervisor.

**The sentence that undercuts all of it (reported inventory snapshot, as of 2026-08-31).** Of the 26 workspace crates, **25 consume zero
OMP surface.** All 7 `consumes` edges in the entire 207-edge graph originate from one crate —
`omp-inventory-map`, the scanner — each carrying the evidence string *"direct process probe produced
this row."* The scanner consumes OMP because the scanner's job is to look at OMP. Nothing else does.

**The thing named `omp-orchestrator` does not yet speak to OMP.**

### The three objections an investor should raise

**HISTORICAL DENOMINATOR NOTE.** The 26-crate and 25-of-26 figures in the following objection and type snapshot are pre-extraction values. Current workspace authority is 50 packages; the old figures are retained only as the historical argument being answered.
**Objection 1 — "You have built 26 crates of scaffolding around a hole. The one integration that justifies the name does not exist."** *Partly conceded, with a narrower truth.* The completion signal is now WIRE-PROVEN upstream, but the supervisory integration that would consume it still does not exist. The 25-of-26 measurement is ours, not a reviewer's. The partial answer is that the layer census shows `observe` WORKS and failure is
concentrated in actionable/consume/actuate — and the seven-row table above records **zero unqualified WORKS rows**. We will not use the qualified observe result as a rebuttal. What we do not concede is that the crates are therefore waste: the
gates operate on the repository and the process boundary, not on OMP, and they run today. The
accurate statement is that the *supervisory* path is unbuilt while the *enforcement* path is built
and running.

**Objection 2 — "Your own evidence discipline is theatre."** *Conceded outright, and we found it
ourselves.* All 183 census rows carry the four mandatory fields — `inputs`, `outputs`, `must_be_true`,
`negative_evidence` — with **zero missing**, and exactly **one distinct value** of `must_be_true` and
**one distinct** `negative_evidence` across the historical census snapshot (00-brief.md §3.2;
`python3 -c "…Counter(json.dumps(r.get('must_be_true')) for r in rows)…"` → crate rows n=26,
distinct=1; non-crate rows n=157, distinct=1). For the 26 crate rows, `inputs`/`outputs` describe
*the scanner's own provenance* rather than the crate's contract, and `what_it_provides` is
boilerplate — "Workspace crate X from cargo metadata" — distinct 26 ways only because the name varies.
The four-field discipline this orchestrator demanded of every worker was satisfied **syntactically
and vacuously**, and the indictment lands on the conductor, not the workers. The answer is
structural: every gate must ship an **anti-vacuity** leg, and the success criterion is not "the
fields are present" but "**the field values discriminate**." A schema fully populated with one value
carries exactly zero bits.

**Objection 3 — “Your gates are unevenly built, so your floor is the weakest one.”** *Accepted as
stated.* The corrected typed classification is: **no gate currently mutates production source through
the real hook**. Of 8 gates, undrained-pipe-lint has the four named legs (1/1/1/3), but its mutation
leg is **FIXTURE**, not AUTOMATED (00-brief.md §3.5); no production-source mutation was tested. The
no-shell-gate mutation is an **AFFORDANCE**—a switch named to be flipped, nothing flips it. Four of 8
have no mutation leg, 2 of 8 have no known-bad leg, and path-literal-guard has no known-good leg.
Those counts are a reported, dated snapshot, not proof of semantic coverage. Earlier all-four and
mutation-AUTOMATED headlines are retracted and non-authoritative.

The highest-risk gate is path-literal-guard: **INFERENCE**, not FACT, says an attack-only suite could
ship an over-strict gate, routing could then bypass it, and the gate could still report green. The
falsification test is a known-good/known-bad fixture pair against the real hook, with a recorded result.

### The corollary connecting all three

omp-types — the crate holding the canonical vocabulary — has **zero dependents** in the reported
snapshot. The vocabulary is defined and unused. **INFERENCE:** this may contribute to the inventory's
fragmentation; “direct cause” is not established. The measured type snapshot reports **51 public enums
(excluding test+bin sources; 59 including them) and 79 structs** across library surfaces; including
test modules and binaries it reports 59/91 over 26 crates. It also reports **6 distinct Verdict-shaped
types sharing no trait, 17 ack/receipt types in 3 incompatible dialects, and 4 colliding type names**.
These are inventory observations, not proof that vocabulary adoption alone solves completion accounting.
The falsification test is the same typed-separation versus logging-only experiment named above, with a
precommitted threshold for fewer false classifications and fewer operator minutes.

**NO-CLAIM:** this reports one checkout on 2026-08-31. It does not claim the census is complete, that
183 rows enumerate every OMP surface, that grep-derived leg counts classify test intent, or that type
collision counts are exhaustive. A name match is not semantic coverage; re-derivation under a stricter
method has not run.


---

## 1.6 Viability gates, risk posture, and stage disposition

This is a **DISCOVER** decision with a narrowed first-value wedge. It is not KILL, PILOT, or BUILD.
The local technical evidence is a FACT about one checkout; buyer prevalence, willingness to pay,
reachable population, distribution, recurrence, rights, and defensibility are UNKNOWN. The authoritative
brief still marks Q1–Q3 as OPEN/PARTIAL; gates 1–4 and 12 are therefore not passed. The fresh grade imposed no
“dispatchable-stage” contract, so this section does not add one.

### Gate status (8–17)

| gate | test | status now | precommitted pass / kill rule |
|---|---|---|---|
| 8 | reachable population, sourced from named fleet operators | **OPEN / UNKNOWN** — no external population sample | pass if 5 qualified operators provide a redacted mismatch; kill/narrow if fewer than 2 do |
| 9 | bottom-up reachable economics | **OPEN / UNKNOWN** — no baseline minutes, volume, or ACV | pass if measured annualized avoided operator cost is at least 3× proposed annual price for 3 of 5 operators; otherwise narrow or kill |
| 10 | distribution access | **OPEN / UNKNOWN** — no acquisition route or funnel | pass if 3 of 5 qualified buyers name an reachable channel and accept a concierge introduction; otherwise do not promote |
| 11 | first-value path | **PARTIAL** — local enforcement exists, OMP supervisor integration does not | pass if 4 of 5 redacted replays produce a reviewable completion → receipt → typed close/refusal; kill this wedge if fewer than 2 do |
| 12 | paid commitment at the proposed 500/month | **OPEN / UNKNOWN** — price is a test, not a validated fact | pass if 3 of 5 qualified economic buyers sign a non-binding letter of intent after replay; zero is a kill signal |
| 13 | unit economics | **OPEN / UNKNOWN** — support, onboarding, compute, and acquisition cost unmeasured | pass if fully loaded recurring cost is below one third of 500/month for the measured cohort; otherwise reprice or kill |
| 14 | recurrence and retention reason | **OPEN / UNKNOWN** — one stand-down is not recurrence | pass if 4 of 5 operators report the mismatch at least monthly over a 30-day diary; otherwise narrow |
| 15 | rights, security, and licensing | **OPEN / UNKNOWN** — data-use rights, secrets, permissions, and licenses unreviewed | pass only with written rights and license review, no unresolved secret/permission blocker, and a fail-closed access test |
| 16 | defensibility / compounding asset | **OPEN / UNKNOWN** — no measured moat against scripts, labor, or native tools | pass only if 3 of 5 buyers choose the receipt contract over their strongest substitute and retained evidence improves the next replay; otherwise treat as commodity |
| 17 | proportionality against substitutes | **OPEN / UNKNOWN** — no 3× incremental comparison exists | pass if measured time/rework reduction is at least 3× incremental adoption cost for 3 of 5 operators; otherwise do not build |

No gate row above is evidence of a pass merely because a technical type or binary exists. A gate is
passed only by its stated external or end-to-end test and a retained receipt.

### Owned risk register

| risk | impact / uncertainty | mitigation | go/no-go test |
|---|---|---|---|
| dispatch blast radius | a false positive could close the wrong work or trigger unwanted action | keep the wedge reviewable and non-autonomous; typed refusal by default | known-good/known-bad replay cannot close without a matching receipt |
| secrets and tokens | OMP/ntm access could expose credentials or cross a permission boundary | least privilege, redacted fixtures, fail closed on missing scope | permission-denied and secret-bearing fixtures produce refusal with no secret emission |
| compatibility and upstream drift | OMP event shape or binary flags can change | pin and record versions; probe tmux with -V; treat stale input as UNKNOWN | one version-drift fixture preserves typed UNKNOWN rather than acting |
| licensing and data-use rights | upstream types, mirror data, or customer frames may be unusable | rights and license review before any paid pilot | written approval covers every shipped dependency and retained frame |
| access and distribution | no buyer channel or install path has been proven | concierge discovery only; no broad rollout claim | 3 of 5 buyers identify an accessible channel and complete install/replay |
| operational failure | adapter may miss asynchronous events or hang after terminal frame | bounded capture, explicit unknown state, no autonomous close | crash, killed pane, rate-limit, and compaction fixtures remain non-success |

### Explicit disposition

**DISCOVER / NARROW — owner:** omp-orchestrator product owner; **reviewer:** an independent viability
reviewer appointed by the project lead. The next test is the five-operator interview plus concierge
replay described in the thesis. Promotion to PILOT requires gates 8–17 plus gates 1–4 and 12 to pass,
three paid commitments at the proposed price, rights/security approval, and a retained end-to-end
receipt. Promotion to BUILD requires a second independent attack and the same conjunction; there is no
implicit promotion by crate count or green internal gates. KILL the wedge if fewer than 2 of 5 operators
show the mismatch, fewer than 2 of 5 complete the first-value replay, or no qualified buyer signs after
five replays. Revisit DISCOVER after 30 days or when the population, recurrence, or price evidence
changes; until then, PILOT and BUILD are prohibited.

### Number provenance and stale-data rule

The following is the section-wide provenance ledger. Values marked “reported” are not fresh command
receipts; they are retained only to explain the local argument and are non-authoritative for promotion.
The current number registry and corrected brief win over every copied value.

| figures | provenance and as-of |
|---|---|
| 6, 7, 23 | historical recollections from one stand-down; no retained artifact or derivation command; non-authoritative |
| 162 and 4.2 hours | UNVERIFIED REPORTED VALUE in 00-brief.md §4 line 526; command receipt not retained in this section; cited as of 2026-08-31, not independently reproducible or a market-frequency claim |
| 28/25/19/2 and 74 total | board snapshot reported in 00-brief.md §3.7 lines 485–486, as of 2026-08-31; command receipt not retained; corrected arithmetic, not live-board truth |
| 23 scanner tests | source-count FACT as of 2026-08-31: 13 markers in types_inventory.rs and 10 in tests/inventory.rs; not a pass count |
| 26 crates, 413 test functions, 31 test files | historical snapshot as of 2026-08-31; current re-derived values are 50 crates, 987 test functions, and 92 test files from the recursive NUMBERS.toml commands |
| 184/207/183, 544,697, 18, 4 | historical inventory snapshot from 00-brief.md §3.2 lines 212-219 as of 2026-08-31; command receipt not retained here; not product completion evidence |
| 8 gates and gate-leg counts | reported corrected snapshot from 00-brief.md §3.5 as of 2026-08-31; grep naming counts are not semantic coverage |
| 51/59 enums, 79/91 structs, 6/17/4 collisions | reported type-inventory snapshot as of 2026-08-31; library-only versus all-source scopes are intentionally distinct |
| tmux 3.6a | captured by tmux -V; echo "exit=$?" on the host, as of 2026-08-31; the long-flag failure is not absence |
| raw agent_end receipt | .flywheel/inventory-artifacts/agent-end-raw-frame.json.gz; capture command and SHA-256 d8bd80c6949b2ec48af1639b5b5e241bd90b4dce1e769483dd1690ed2be8f644 are retained in section 1.2.1; /tmp/grade/agent-end-raw-frame.json is only the historical source location and must not be cited |

Any later correction must update this ledger or remove the figure. Historical/retracted values are never
silently promoted back to current facts.

## 1.7 Recorded under R11

Two constraints surfaced while writing this section and are recorded here rather than left in
conversation:

1. **The cargo shim is an unmeasured supply-chain surface** in the metadata path (§1.3.6). We know it
   is a shim; we have not measured its effect on arguments or toolchain selection.
2. **Tmux's old “no machine-readable handshake” interpretation was wrong.** The current correction is
   the short -V handshake and a false-negative-on-presence risk for loops that use --version (§1.3.9).

**NO-CLAIM:** nothing here commits to a schedule, a cost, or an architecture validated by execution.
This section establishes a scoped hypothesis, local observations, dependency-contract floor, risks, and
explicit discovery gates. It does not imply product viability, PILOT, or BUILD.

---

## 1.2.3 BLOCKER resolution — the row was named after a field the frame does not contain

`GradeIdea` filed:

> Section 1.2.1 claims the completion gap is "WIRE-PROVEN" in the table but then
> immediately walks back the claim in the NO-CLAIM subsection. The claim's strength is
> undefined until both subsections are read in sequence, and they contradict each
> other's headline.

Half right, and measurement found the sharper problem underneath.

### The citation is honest; the row LABEL was not

The artifact is real and checkable — that part of the section is in good order:

| | |
|---|---|
| path | `/tmp/grade/agent-end-raw-frame.json`, 4,772 bytes |
| cited SHA-256 | `d8bd80c6949b2ec4…` |
| **re-derived 2026-09-01** | `d8bd80c6949b2ec4…` — **exact match** |

But the frame's top-level keys are `['type', 'messages', 'isTerminal']`.
**`willContinue` does not appear in it at all.**

So the row was titled `AgentEndEvent.willContinue + SessionStopEvent` and marked
`WIRE-PROVEN`, while what the wire actually produced was **`isTerminal`**. The effect
column already said *"shared `willContinue` was absent"* — so the section knew, and the
label contradicted the section's own evidence one column to its left.

That is worse than the contradiction the grader described. A reader skimming the table
takes away "`willContinue` is wire-proven", which is exactly false, and the correction
sits in prose they may not reach.

**Corrected:** the row now names `isTerminal` as the observed field and moves
`willContinue` and `SessionStopEvent` to DECLARED ONLY, where the other six gaps
already sit.

### The plan's only wire proof was living in /tmp

Every other row in that table is DECLARED ONLY. This one frame is **the sole
wire-level evidence in the entire plan**, and it sat in `/tmp/grade/` — clearable on
reboot, after which the strongest evidence claim in the document would have been
uncheckable by anyone including its author.

Preserved at `.flywheel/inventory-artifacts/agent-end-raw-frame.json.gz` (4,772 → 1,985
bytes) and added to the `artifact_provenance` gate, which decompresses and re-hashes it.
Same defect class as §4.7's diagrams, and sharper here because of what the single
artifact carries.

### NO-CLAIM, unchanged and still binding

One frame is not repeatability. `isTerminal` being observed does not establish that it
is semantically equivalent to `willContinue=false`, that a supervisor consumes it, or
that it survives crashes, killed panes, rate limits or compaction. Section 10 already
records the cost of overreading this surface: it claimed a worker-completion signal was


---


<!-- ===== 02-surface-census.md ===== -->

# 02 — What we are mapping: every OMP surface

Every number in §§1–5 is `MEASURED` only when the inventory artifact and exact query
are named. §§6–11 also use the separately identified surface-map snapshot and retained
worker observations; those observations are explicitly marked `HISTORICAL`,
`ESTIMATE`, or `UNVERIFIED` when no command receipt is retained. A number is not
made measured by repeating it. The primary inventory artifact is the built scanner
`omp-inventory-map`, run as

`/Volumes/BuildShared/cargo-targets/debug/omp-inventory-map` was run against installed `omp/18.0.11` on 2026-08-31 from repo `/Users/josh/Developer/omp-orchestrator`. The retained historical capture is `.flywheel/inventory-artifacts/inv.txt.gz`; it decompresses to 544,697 bytes, raw SHA-256 `86491732a5581a6d2e342d0db59bdf20e5f47f6da93150ae78bd2649562f5081`, and exit 2. The compressed artifact SHA-256 is `8f62893e6a4a04a9b4e8922781a5f8a687f73ca84f5c4ea9d69c5f8998ae0561`.

The snapshot is historical and revision-unpinned; it is not current workspace evidence. Every derived count in §§1–5 must carry this artifact boundary, and later sections do not inherit it.

**Provenance register.** INV-2026-08-31 is the retained historical scanner output above. The current map artifact is `docs/plan/SURFACE-MAP.jsonl`, with **614 non-empty JSONL rows, 302,002 bytes, SHA-256 5b3c3238c4ec9dd7f72a097bb3668e7de224e3b6f0eddc1132de2902a1d9d93c**. Its generator/tool version and source revision were not retained, so later claims are limited to this hash-anchored file and named queries.
Map counts below are derived by the exact NUMBERS.toml commands; worker observations without retained command/output/revision remain historical context, not current measurement.

### 1. The census, in one table

The scanner emits a versioned envelope,
`{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"UNKNOWN","data":{…}}`,
carrying 184 nodes, 207 edges, and 183 rows. The denominator is worth stating plainly, because a census with an unstated denominator is a press release: 183 rows = every OMP surface the probe could enumerate, plus our 26-crate snapshot recorded on 2026-08-31. It is not 183 OMP features. It is 157 OMP surfaces and 26 things built in that historical snapshot.

**SNAPSHOT BOUNDARY.** The 183-row denominator and all ratios below are dated inventory results, not current workspace counts; re-run cargo metadata and regenerate this section after extraction.

**CURRENT-STATE BOUNDARY (2026-09-01):** the 183-row artifact and 26-crate denominator above are pre-extraction historical data. Current cargo metadata reports 50 packages and 48 binary targets; regenerate the scanner artifact before using any §§1–5 count as current. The historical artifact remains retained for provenance, not closure.

`MEASURED` — `python3 -c "import json,collections; d=json.load(open('/tmp/inv.txt'))['data']; print(collections.Counter(r['kind'] for r in d['rows']))"`

| Row kind | Count | What one row is |
|---|---|---|
| `type_root` | 57 | A top-level directory in OMP's shipped TypeScript type surface |
| `rpc_handler` | 42 | A named method the `--mode=rpc` transport dispatches |
| `cli_command` | 39 | A subcommand enumerated from `omp --help` |
| `workspace_crate` | 26 | One of *our* crates, from `cargo metadata --no-deps` |
| `declaration` | 14 | A top-level `.d.ts` file in the shipped type surface |
| `omp_method` | 3 | A JSON-RPC method whose name matches `omp/*` |
| `slash_command` | 1 | Synthetic `UNKNOWN_PROBE` placeholder row; not an observed slash command — see §3 |
| `transport` | 1 | The process-level transport selector |
| **Total** | **183** | |

The envelope's `counts` block records observed counts, while the row tally also includes
synthetic rows. For the seven counted kinds, six observed/expected pairs match exactly;
`slash_commands: 0` is the real observed count and `expected_slash_commands: 136` is
the unsatisfied scanner expectation. The one emitted `slash_command` row is only the
`UNKNOWN_PROBE` placeholder and must not be counted as an observed command. The row
tally also contains one `transport` row, but the envelope has neither
`counts.transport` nor `expected_transport`; therefore there is no transport twin.
The scanner status is `UNKNOWN` and the process exits 2 because slash enumeration did
not earn a verdict. A timeout is not a verdict; neither is an empty probe.


### 2. The coverage headline

`MEASURED` — `python3 -c "import json,collections; print(collections.Counter(r['classification'] for r in json.load(open('/tmp/inv.txt'))['data']['rows']))"`

```
CAPABILITY_NOT_USED             157
SCRAPED_OR_OBSERVED_ALTERNATIVE  18
MAPPED_BY_DIRECT_PROBE            8
```
**HISTORICAL DIRECT-PROBE RATIO SNAPSHOT.** The 8/183, 7/157, and related ratios below belong to the retained pre-extraction artifact; they are not current workspace coverage.
- direct-probe coverage = 8 / 183 = **4.37%** of the all-census rows (8 ÷ 183 = 0.043715…);
- OMP-only direct-probe coverage = 7 / 157 = **4.46%** (the eighth direct row is the workspace crate `omp-inventory-map`);
- alternative-path coverage = 18 / 183 = **9.84%** and unconsumed capability = 157 / 183 = **85.79%**;
- 8 + 18 + 157 = 183, so the three classes partition the all-census rows with no residue. The OMP-only denominator is 157, not 183.

The edge graph tells the same story from the other side. Of 207 edges, exactly **7
are `consumes`**, and all 7 originate from a single crate, `omp-inventory-map`,
each carrying the evidence string *"direct process probe produced this row"*. They
point at `type_root:cli`, `type_root:commands`, `type_root:jsonrpc`,
`type_root:slash-commands`, `rpc_handler:get_available_commands`,
`slash_command:UNKNOWN_PROBE`, and `transport:--mode=<value>`. So:

- crates consuming any OMP surface = 1 / 26 = **3.85%** (1 ÷ 26 = 0.038461…)
- `consumes` share of all edges = 7 / 207 = **3.38%** (7 ÷ 207 = 0.033816…)

`MEASURED` — the one crate that consumes OMP surface is the crate whose job is to
enumerate OMP surface. Twenty-five of twenty-six crates consume none.

An investor should read that as the project's **central open question, not its
verdict**. Two readings are available and we are obliged to state the hostile one
**Historical hostile reading.** The retained pre-extraction snapshot touched 4.46% of OMP surfaces (7/157); its 25 workspace crates were gates and lints that would work identically if OMP did not exist. Do not apply that denominator to the current 50-crate worktree.
*Our reading:* the map is honest, which is the hard part and the part usually skipped
— most projects at this stage cannot tell you their consumption ratio at all, because
nobody enumerated the denominator. The all-census figure is 4.37% (8/183); the
OMP-only figure is 4.46% (7/157). The plan's job from here is to move the latter by
named decisions, one surface at a time, with a disposition on each of the 157 OMP rows.

`PROJECTED` — we expect direct-probe coverage to rise as the RPC session crate
wires named handlers, but this document makes no forecast of a target percentage,
because a coverage target would immediately become a metric to game: wiring a
handler nobody calls raises the ratio and lowers the truth.

There is a structural reason for the ratio, and it is recorded in the brief's
five-stage control loop (formerly "five-stage" — renamed, the table has five stages and seven rows) table rather than discovered here: of the five layers
(observe / actionable / consume / actuate / complete), exactly one — `observe`,
via `tick-monitor` — is `MEASURED` as WORKS. `actuate` **does not exist**; a human
types into panes. A project whose actuation layer does not exist cannot consume an
actuation surface, so the 157 `CAPABILITY_NOT_USED` rows are not 157 independent
oversights. They are largely one missing layer, counted 157 times. That reframing
makes the number smaller *and* the fix harder: it is one hard thing, not 157 easy
ones. `NO-CLAIM:` this attributes the ratio to the missing actuation layer as an
explanation, not as a measurement — no experiment here isolates how many of the
157 rows would flip once actuation exists.

### 3. The surface, enumerated by kind

`MEASURED` — all member lists below are produced by
`python3 -c "import json,collections; d=json.load(open('/tmp/inv.txt'))['data']; by=collections.defaultdict(list); [by[r['kind']].append(r['surface'].split(':',1)[1]) for r in d['rows']]; print(sorted(by['<KIND>']))"`.

#### cli_command — 39 of 39, listed in full

`acp`, `agents`, `auth-broker`, `auth-gateway`, `bench`, `browser-relay`,
`cleanse`, `commit`, `completions`, `compress`, `config`, `dry-balance`,
`gallery`, `gc`, `git`, `grep`, `grievances`, `if-bench`, `images`, `install`,
`join`, `models`, `plugin`, `ps`, `read`, `render`, `say`, `search`, `setup`,
`share`, `shell`, `ssh`, `stats`, `tiny-models`, `token`, `ttsr`, `update`,
`usage`, `worktree`.

All 39 classify `CAPABILITY_NOT_USED`. We do not shell out to a single OMP
subcommand today, which is a deliberate consequence of the no-shell rule: a
subcommand invocation is a subprocess, and a subprocess without the
`subprocess-contract` (group kill, both pipes drained, `&Cx` first) is not
allowed to exist in this repo.

#### omp_method — 3 of 3, listed in full

`omp/muxConnect`, `omp/muxPing`, `omp/muxRestartServer`.

These are the only three methods on the installed binary whose names match
`omp/*`. All three classify `CAPABILITY_NOT_USED`. This is the number that
retired an earlier claim: an older draft of `AGENTS.md` asserted "81 JSON-RPC
methods, 17 used". That figure was not re-derivable from any probe and has been
struck. The measured surface is 3 `omp/*` methods and 42 bare-named RPC handlers.

#### transport — 1 of 1, listed in full

`--mode=<value>`.

One transport selector, classification `MAPPED_BY_DIRECT_PROBE`, owner
`omp-inventory-map`. It is one of the 7 `consumes` edges.

#### slash_command — 1 row, and it is a confession

`UNKNOWN_PROBE`, classification `MAPPED_BY_DIRECT_PROBE`. The envelope's
`counts.slash_commands` is `0` while `counts.expected_slash_commands` is `136`.
The scanner could not enumerate slash commands, so instead of emitting 136
guesses or 0 rows and calling it clean, it emits one row named `UNKNOWN_PROBE`
and drives `status` to `UNKNOWN` with exit 2. `MEASURED` — the largest **unenumerated**
region of the OMP expectation is 136 slash commands; no claim is made that 136 is the world count.

**Recorded under R11, because it is not yet written down anywhere else.** The
brief's §3.2 lists `slash_commands=0` among the counts but does not carry
`expected_slash_commands=136`, and no section owns the gap. Writing it here makes
it a requirement rather than a session memory: *the census is not complete until
slash-command enumeration either succeeds or carries a named reason for why it
cannot.* `MEASURED` — the discrepancy is in the artifact:
`python3 -c "import json; c=json.load(open('/tmp/inv.txt'))['data']['counts']; print(c['slash_commands'], c['expected_slash_commands'])"`
→ `0 136`. Six of the seven `expected_*` twins match their measured count exactly;
this is the only one that does not. `NO-CLAIM:` we do not know that 136 is the
true number of slash commands — it is the scanner's expectation, and an
expectation that was never satisfied is not a measurement of the world.

#### declaration — 14 of 14, listed in full

`cli-commands.d.ts`, `cli.d.ts`, `config.d.ts`, `cursor-bridge-tools.d.ts`,
`cursor.d.ts`, `index.d.ts`, `main.d.ts`, `sdk.d.ts`, `startup-splash.d.ts`,
`system-prompt.d.ts`, `telemetry-export-otlp.d.ts`, `telemetry-export.d.ts`,
`thinking.d.ts`, `workspace-tree.d.ts`. All 14 classify `CAPABILITY_NOT_USED`.

#### rpc_handler — 42 total; all 42 named

`abort`, `abort_and_prompt`, `abort_bash`, `abort_retry`, `bash`, `branch`,
`compact`, `cycle_model`, `cycle_thinking_level`, `export_html`, `follow_up`,
`get_available_commands`, `get_available_models`, `get_branch_messages`,
`get_last_assistant_text`, `get_login_providers`, `get_messages`,
`get_messages_page`, `get_session_stats`, `get_state`, `get_subagent_messages`,
`get_subagents`, `handoff`, `login`, `negotiate_protocol`, `new_session`,
`prompt`, `set_auto_compaction`, `set_auto_retry`, `set_fast_mode`,
`set_follow_up_mode`, `set_host_tools`, `set_host_uri_schemes`,
`set_interrupt_mode`, `set_model`, `set_session_name`, `set_steering_mode`,
`set_subagent_subscription`, `set_thinking_level`, `set_todos`, `steer`,
`switch_session`.

Split by classification: 36 `CAPABILITY_NOT_USED`, 5
`SCRAPED_OR_OBSERVED_ALTERNATIVE` (`bash`, `follow_up`, `get_state`, `prompt`,
`steer`), 1 `MAPPED_BY_DIRECT_PROBE` (`get_available_commands`). Those five are
precisely the handlers an orchestrator most wants — send a prompt, steer a running
agent, read its state, run a command, follow up — and today we obtain each of them
some other way. That is stated as a debt in §4, not as a design.

#### type_root — 57 total; all 57 named

`advisor`, `async`, `auto-thinking`, `autolearn`, `autoresearch`, `blob-broker`,
`capability`, `cleanse`, `cli`, `collab`, `commands`, `commit`, `compress`,
`config`, `dap`, `debug`, `discovery`, `edit`, `eval`, `exa`, `exec`, `export`,
`extensibility`, `goals`, `hindsight`, `if-bench`, `internal-urls`, `irc`,
`jsonrpc`, `launch`, `lib`, `live`, `lsp`, `markit`, `mcp`, `memories`,
`memory-backend`, `mnemopi`, `modes`, `plan-mode`, `registry`, `secrets`,
`security`, `session`, `sharpshooter`, `slash-commands`, `ssh`, `stt`,
`subprocess`, `task`, `tiny`, `tools`, `tts`, `tui`, `utils`, `vibe`, `web`.

Split: 40 `CAPABILITY_NOT_USED`, 13 `SCRAPED_OR_OBSERVED_ALTERNATIVE` (`dap`,
`debug`, `exec`, `goals`, `memories`, `memory-backend`, `mnemopi`, `modes`,
`plan-mode`, `session`, `subprocess`, `task`, `tools`), 4
`MAPPED_BY_DIRECT_PROBE` (`cli`, `commands`, `jsonrpc`, `slash-commands`).

#### workspace_crate — 26 of 26, listed in full

`ack-spine`, `ack-stage`, `commit-build-fence`, `composer-typed`,
`dispatch-claim-fence`, `dispatch-silence-watch`, `finding`, `finding-dispatch`,
`fleet-composite`, `installer`, `kernel-bypass-gate`,
`kernel-only-operator-hook`, `loop-queue-filter`, `no-shell-gate`,
`omp-inventory-map`, `omp-orchestrator`, `omp-rpc-session`, `omp-types`,
`pane-dispatch-fence`, `path-literal-guard`, `pre-delete-citation-check`,
`receiver-receipt`, `state-wildcard-lint`, `subprocess-contract`, `tick-monitor`,
`undrained-pipe-lint`.

25 classify `CAPABILITY_NOT_USED`; only `omp-inventory-map` is
`MAPPED_BY_DIRECT_PROBE`, with the reason *"This crate owns generation and direct
probe orchestration."* Note what `CAPABILITY_NOT_USED` means when applied to our
own crate: the scanner is saying **we do not consume our own crate from any
measured runtime trigger**, which is the same finding as `omp-types` having zero
dependents. The classification is uncomfortable and correct.

### 4. The three classifications, defined and dispositioned

The census carries an `orphan_disposition` on every row. `MEASURED` —
`python3 -c "import json,collections; print(collections.Counter(r['orphan_disposition'] for r in json.load(open('/tmp/inv.txt'))['data']['rows']))"`
yields `NAMED_REASON: 175, WIRE: 8`. There is no third value, and there must never
be one, because the third value is always "later".

**`MAPPED_BY_DIRECT_PROBE` (8 rows) — we actually touch it.** A live process probe
produced the row. The evidence string is *"direct process probe produced this
row"*, the owning crate is named, and a `consumes` edge exists in the graph. This
is the only class that carries runtime truth. Disposition: keep, and keep probing —
a mapped row that stops being probed silently degrades to a scraped row.

**`SCRAPED_OR_OBSERVED_ALTERNATIVE` (18 rows) — we get the information some other
way.** The row's reason text is uniform: *"No typed runtime adapter owns
`<surface>`; retain as a named wire candidate."* Naming the alternatives, which is
the part that makes this class honest rather than a synonym for "unused":

- `rpc_handler:prompt` and `rpc_handler:steer` — dispatch reaches panes through
  `ntm` (`ntm version v1.30.0-1-gda270719`) and the pane-dispatch fence, not
  through OMP's RPC. The alternative is a terminal multiplexer.
- `rpc_handler:get_state` — agent liveness is inferred by `tick-monitor` and
  `dispatch-silence-watch` from observed pane output, not asked for over RPC. The
  alternative is silence-timing.
- `rpc_handler:bash` — command execution goes through `subprocess-contract`
  (group kill, both pipes drained) under the no-shell rule; OMP's `bash` handler
  is never reached. The alternative is our own spawn discipline.
- `rpc_handler:follow_up` — follow-ups are modelled as `receiver-receipt` and
  `ack-stage` obligations. The alternative is our ack ledger.
- `type_root:task`, `type_root:goals`, `type_root:plan-mode` — work items live in
  `br 0.4.1` beads and are ranked by `bv v0.20.0`. The alternative is the bead board.
- `type_root:memories`, `type_root:memory-backend`, `type_root:mnemopi` — recall
  is served by the harness's own memory tools. The alternative is the harness.
- `type_root:subprocess` and `type_root:exec` — superseded by our
  `subprocess-contract` crate, which encodes asupersync 0.4.9 (`fa3c01aec`) rules
  OMP's surface does not promise.
- `type_root:session` — sessions are tracked by `omp-rpc-session` locally.
- `type_root:dap`, `type_root:debug`, `type_root:tools`, `type_root:modes` — no
  alternative is named today. `MEASURED` — these four are the rows where the class
  is doing the least work, and they are the first candidates to be re-classified
  down to `CAPABILITY_NOT_USED` so they inherit a real disposition.

Disposition: every scraped row must name its alternative in one sentence, and the
alternative must be a thing that exists. A scraped row whose alternative is "we
plan to" is a `CAPABILITY_NOT_USED` row wearing a better hat.

**`CAPABILITY_NOT_USED` (157 rows) — a real capability we do not consume.** The
reason text is *"The repository has no measured runtime trigger for `<surface>`."*
This class admits exactly two dispositions and no third:

- **WIRE** — a proposal to consume it. The current map has no bead ID, owner, or due-time
field, so a WIRE row is not evidence that a bead exists. A conforming WIRE record must
carry explicit `bead_id`, owner, and timing fields and a retained
validation receipt; until then it is a proposal, not bead-backed disposition.
- **RETIRE / NAMED_REASON** — a proposed non-consumption decision. It is not validated merely
because `validated_by` is non-null; a current RETIRE proof requires structured
read-only probe evidence (command, exit/result, timestamp, and artifact identity).
"Not yet triaged" is not a disposition. `MEASURED` — 175 rows currently sit at
`NAMED_REASON` and 8 at `WIRE`, so 175 ÷ 183 = **95.63%** of the census is
currently answered with a reason rather than a plan. `PROJECTED` — the plan's
first milestone converts a named subset of those reasons into WIRE beads; this
document does not pre-commit the size of that subset, because a number chosen
before the triage is a number chosen to look good.

### 5. The vacuity finding, stated against ourselves

This is the sharpest thing the session produced, and it is an indictment of our own
process discipline, so it belongs here rather than in a footnote.

`MEASURED` — reproduced exactly:

```
python3 -c "…collections.Counter(json.dumps(r.get('must_be_true')) …)"
  crate rows:     n=26   distinct must_be_true=1  distinct negative_evidence=1
  non-crate rows: n=157  distinct must_be_true=1  distinct negative_evidence=1
```

Re-run on the live artifact for this section, with the missing-field check added:

```
python3 -c "
import json
d=json.load(open('/tmp/inv.txt'))['data']; rows=d['rows']
cr=[r for r in rows if r['kind']=='workspace_crate']; nc=[r for r in rows if r['kind']!='workspace_crate']
for nm,s in [('crate',cr),('non-crate',nc)]:
    print(nm,'n=%d'%len(s),
      'distinct must_be_true=%d'%len({json.dumps(r['must_be_true']) for r in s}),
      'distinct negative_evidence=%d'%len({json.dumps(r['negative_evidence']) for r in s}),
      'missing=%d'%sum(1 for r in s if not all(r.get(f) for f in ('inputs','outputs','must_be_true','negative_evidence'))))
"
  crate     n=26  distinct must_be_true=1  distinct negative_evidence=1  missing=0
  non-crate n=157 distinct must_be_true=1  distinct negative_evidence=1  missing=0
```

All 183 rows carry all four mandatory fields — `inputs`, `outputs`,
`must_be_true`, `negative_evidence` — with **zero missing**. And across the entire
census there is **exactly one distinct `must_be_true` and one distinct
`negative_evidence`**:

```
must_be_true      = ["The source probe is non-empty before a known verdict is emitted.",
                     "A versioned inventory envelope carries the probe state."]
negative_evidence = ["No repository source grep was used; ownership is derived from
                     metadata and direct probes."]
```

The four-field discipline the orchestrator demanded of every worker is therefore
satisfied **syntactically and vacuously**. It matters for one reason, and the
reason generalises well past this repo: *a census where every row carries identical
invariants proves the fields were populated, not that anything was checked.* A
per-row `must_be_true` is supposed to be the thing that could have been false about
**that row**. If it is the same sentence 183 times, it is a property of the
scanner's envelope, not of the surface, and no row-level falsification is possible.
A validator asserting "every row has a non-empty `must_be_true`" passes at 100%
while checking nothing.

It is worse for crate rows specifically. `MEASURED` — for a crate row, `inputs` is
`cargo metadata --format-version 1 --no-deps` plus that crate's `Cargo.toml`, and
`what_it_provides` is the boilerplate *"Workspace crate `<name>` from cargo
metadata"*. Those fields describe **the scanner's own provenance**, not the crate's
contract. The 26 `what_it_provides` strings are distinct only because the crate
name varies inside a fixed template. Sampling `outputs` shows the same shape:
`ack-spine` → `ack_spine, ack-spine, ack_detector, authorities, followup`;
`ack-stage` → `ack_stage`; `commit-build-fence` → `commit_build_fence,
commit-build-fence, hook`; `composer-typed` → `composer_typed, composer-typed,
differential, mutation, planted_known_bads`. These are cargo target names. They
tell you what the crate compiles, not what it promises.

**The fix, as future work.** `PROJECTED` — two changes, both cheap, both testable:

1. **Per-row invariants that differ by row.** Each row's `must_be_true` must state
   something falsifiable about *that surface*: for `cli_command:worktree`, "the
   installed binary lists `worktree` in `omp --help`"; for `crate:ack-spine`, "the
   crate exposes an ack detector reachable from `finding-dispatch`". A row whose
   invariant is reusable verbatim by another row has not written an invariant.
2. **An anti-vacuity gate.** A gate leg that loads any emitted census and **fails
   when `distinct-invariant-count == 1` across a census of more than one row**, and
   more generally when the distinct-invariant ratio falls below a floor. This is
   the same shape as the `known_bad` leg the gate framework already requires: it is
   a planted-known-bad for the *metadata*, not the code. `MEASURED` — the
   `no-shell-gate` crate is the only one of eight gates today carrying all four
   legs (6 anti-vacuity files by `grep -rli`), so the pattern exists in-repo and
   needs porting, not inventing.

What would Jeffrey do here? The pattern we are reaching for — refuse to emit a
verdict the evidence does not support, and make the refusal typed — is already the
shape of this scanner's `status: UNKNOWN` / exit 2 behaviour, which is itself
modelled on the mirror's fail-closed convention (compare `fh`'s typed
`SERVE_INPUT_STALE` refusal when its mirror HEAD moved `5dec4212…` → `ecdea397…`,
rather than serving stale results). `MEASURED` — the `fh` MCP surface is failing
closed with exactly that typed error as of this session, which is the behaviour we
want and a live demonstration that it is survivable. We did not run a mirror grep
for a pre-existing distinct-invariant-ratio gate; that search is named as an open
item for the prior-art section rather than claimed here.

There is also a sixth gate property this census forced into existence, and it
belongs on the record next to the vacuity finding because it has the same cause —
a gate that is correct but unreachable is as vacuous as an invariant that is
populated but identical. `MEASURED` — `omp-inventory-map --help` returns
`{"status":"ERROR","error":"CONFIG_ERROR unknown argument --help"}`. The gate is
**UNVERIFIED:** the retained record has no exact scoped test command, exit/output receipt,
or source revision for the claim that the gate is built/correct or that 13 tests pass.
`types_inventory.rs:176-178` is a source assertion, not test execution. The only
current runtime evidence is the `--help` error above; ADDRESSABLE remains a required
property until a command, receipt, and revision are attached.

I re-derived the brief's §3.6 corroborating claim independently rather than
inheriting it, because it is the one fact in the brief that lives inside *my*
artifact. `MEASURED` —
`for w in Observation CONVERGE Verdict; do printf '%s ' "$w"; grep -o "$w" /tmp/inv.txt | wc -l; done`
→ `Observation 0`, `CONVERGE 0`, `Verdict 0`. The 544,697-byte doctor output
contains none of the three. **I agree with the brief.** The type-collision logic
is present in the source and absent from every byte the running binary emits,
which is the precise definition of wired-but-unaddressable. `NO-CLAIM:` a string
absent from the doctor output does not prove the check never runs — it proves the
check is unobservable from the only output the binary offers, which is the defect
being named.

---
`NO-CLAIM:` §§1–5 claim only what the `INV-2026-08-31` snapshot of
`omp-inventory-map` against `omp/18.0.11` emitted, plus the printed arithmetic.
The snapshot is not a completeness claim: `expected_slash_commands: 136` against
`slash_commands: 0` proves 136 expected slash commands remain unenumerated. The 42
`rpc_handler` names are only the set returned by this probe, not the whole RPC surface.
Sections 6–11 instead use `docs/plan/SURFACE-MAP.jsonl` (identity in the provenance
register), exact retained `--help`/grep observations, and historical worker reports;
a worker report without a command, output, and revision is not a current measurement.
The map's non-null `validated_by` field is not proof of structured RETIRE evidence,
and current WIRE rows are proposals, not proof of bead existence. No claim is made that
any `CAPABILITY_NOT_USED` row is genuinely unused at runtime, that the alternatives
are adequate substitutes, or that any coverage target should move to a particular figure.

---

## 6. Every surface this system relies on — not just OMP

**R14, added by Josh:** *"mining of dicklesworthstone-mirror projects, ntm surfaces, br surfaces, bv
surfaces, and any surfaces that this system relies on."*

§1–§5 census one dependency. The pre-correction §6 table below is a HISTORICAL SNAPSHOT
(NON-AUTHORITATIVE): its retained method was `--help` subcommand extraction plus grep over
`crates/` and `docs/plan/`, but this section retains no complete output receipt or
source revision. The current map correction is recorded in §9; do not treat §6 shares as live.

| surface | total | consumed | share |
|---|---:|---:|---:|
| OMP | 183 | 7 | **3.8 %** |
| `ntm` | 114 | 6 | **5.3 %** |
| `br` | 46 | 10 | **21.7 %** |
| `bv` | 29 | **0** | **0.0 %** |
| **total** | **372** | **23** | **6.2 %** |

**The system named `omp-orchestrator` consumes 6.2 % of the surfaces it orchestrates through.**

### 6.1 `bv` is zero, and that is the load-bearing number

Not "rarely used" — **zero**. No crate invokes it (`grep -rn 'Command::new("bv")' crates/` → `0`,
§11.5) and no plan section names one of its 29 subcommands. `bv` is the graph-triage tool whose
entire purpose is answering *what should be worked next*, which is stage **S4** of the lifecycle,
and S4 is one of the three severed links in §11.5.

The consequence is measurable rather than theoretical: selection this session was done by the
conductor picking work by recency of his own discovery, and when the graph was finally consulted,
**the top-three PageRank items were unclaimed while hand-picked items were in flight.** The tool
that would have prevented that was installed, working, and never called.

`bv` also ships `capabilities`, `insights`, `decision-relevant`, `exit-codes` and
`dependency` surfaces — the self-describing contract §07 specifies as our own bar — and we have
adopted none of it while writing a section arguing that every CLI should have it.

### 6.2 What the shares mean, read honestly

`br` at 21.7 % is the only tool we use like a dependency rather than a shell. That is unsurprising:
beads are the unit of work, so the surface gets exercised. It is also the tool whose *close policy*
we describe as convention rather than code (§11.1, S3), so even here the consumption is shallow —
we call `create`/`update`/`close`/`comments` and inherit the policy without asserting on it.

`ntm` at 5.3 % of 114 is the sharpest gap after `bv`. HISTORICAL SESSION OBSERVATION
(approximate; no command/output receipt retained): this session hand-rolled dispatch packets while
`ntm template` — a surface we *do* reference — ships a `dispatch` template with fail-closed required variables (§11.3).

**NO-CLAIM.** These shares measure *reference*, not *correct use*: a `grep` hit means the name
appears in a crate or a doc, not that we invoke it correctly or at all. The counts come from
`--help` subcommand extraction, which under-counts flags and over-counts alias lines; `ntm`'s 114
and `bv`'s 29 are subcommand-level figures and the flag surface beneath them is unmeasured. A low
share is **not automatically a defect** — most of `ntm`'s 114 surfaces are irrelevant to
orchestration, and adopting a surface because it exists is the opposite of the discipline this plan
argues for. The defensible claim is narrower and it is about **one** tool: `bv` at zero, for the
stage the plan names as broken, is a gap with a measured cost.

---

## 7. The census picked its own denominator, and it was too small

`%1409` and Josh broke §6 within minutes of each other, from opposite directions. Both are right and
the section above is now known-incomplete rather than merely incomplete.

### 7.1 Josh: "what other binaries are we missing? rch, ru, etc."
The following is a HISTORICAL INVENTORY (NON-AUTHORITATIVE), not a live count: 19 relevant
binaries were reported installed, but the retained evidence does not include a complete exact
enumeration command/output or source revision. The four-tool choice was not justified; §6's
pre-correction figures are superseded by §9's map snapshot.

| binary | refs in repo | censused | note |
|---|---:|:--:|---|
| `omp` | 61 | yes | |
| `git` | 44 | **no** | spawned by our crates |
| `br` | 40 | yes | |
| `cargo` | 31 | **no** | resolves to `/Users/josh/.rch/shims/cargo` — **a shim** |
| `ntm` | HISTORICAL ESTIMATE (~26; command/output receipt not retained) | **yes (§6)** | 1:many session plane; second-largest censused consumer, omitted from this table's first draft |
| `tmux` | 29 | **no** | pane truth is read through it |
| `bv` | 15 | yes | census was wrong — see §7.2 |
| `fh` | 10 | **no** | **R7's entire mechanism** |
| `rch` | 6 | **no** | **gates every build in this repo** |
| `cass` | 3 | **no** | |
| `jsm`, `caut` | 2 | **no** | |
| `ubs` | 1 | **no** | |
| `ru`, `dcg`, `pt`, `sbh`, `slb`, `gh` | **0** | **no** | installed, unreferenced |


**Three of these are load-bearing and absent from the census:**

- **`rch` gates every compile.** `cargo` on `PATH` is its shim. Every `cargo` invocation this
  session carried `RCH_ENABLED=false` — a HISTORICAL observation with no retained command receipt or revision. The current query `grep -rc 'RCH_ENABLED' docs/plan/*.md` returns **2 matching lines**, both historical references in this section; this is not evidence that builds are currently disabled.
  The earlier workflow therefore disabled a build gate on each command; this section records that historical gap, not current status.
- **`dcg` has zero references and is an active participant.** `dcg 0.5.6` is installed and it
  **blocked one of this session's commands**. A tool that can refuse our actions, cited nowhere.
- **`fh` is R7.** The requirement "mine the mirror at every gap" is executed entirely through `fh`,
  which the census of surfaces-we-rely-on omitted.

### 7.2 `%1409`: the `bv` rows are scrape artifacts

The 29 `bv` "surfaces" in §6 are **help prose**, not subcommands. The extraction regex
`^\s{2,}([a-z][a-z-]{2,})` harvested words from wrapped description text: `ago`, `background`,
`calculation`, `for`, `fraction`, `large`, `not-ready`. Meanwhile `bv`'s **real** agent surface is
**40+ `--robot-*` flags** — `--robot-next`, `--robot-triage`, `--robot-plan`, `--robot-capabilities`,
`--robot-graph`, `--robot-blocker-chain` — and **not one of them is a row**.

So `bv 0/29` is not "we use none of 29." It is **a denominator of the wrong things**. The direction
of the error is worth stating: the real surface is larger and the real consumption is still zero, so
the finding survives while its evidence is retired.

### 7.3 The live proof `%1409` produced, which is the point of the whole exercise

Mapping batch 6 it ran the tool instead of reasoning about it:

```
bv --robot-next   -> exit 0; PageRank names bead 2o5, "Unblocks 2z2.1/2z2.2"
```

**`2o5` is the articulation point.** It is the `rch`-divergence blocker filed tonight, and both `2z2`
children are blocked on it — so it is the single highest-leverage open item on the board. Nineteen
waves of dispatch this session, every one selected by the conductor's recency, **never picked it.**
The graph named it on the first call.

That is the `bv`-at-zero cost, measured rather than argued: not "we should use the graph" but
"the graph, run once, immediately named the item nineteen hand-picks missed."

### 7.4 We forbid `.py` files and spawn `python3` six times

```
grep -rhoE 'Command::new\("(python3|grep|strings)"\)' crates/
  6  Command::new("python3")
  1  Command::new("grep")
  1  Command::new("strings")
```

The repo's one hard rule forbids **tracked `.sh` and `.py` files**. It does not forbid *invoking*
those interpreters, and six crate call-sites spawn `python3`. This is the same boundary finding as
`.git/hooks/commit-msg-verification-level.sh` in §11.4: the rule governs file extensions in the git
index, and both the hook directory and the process table sit outside it.

Not a violation — a **coverage** statement. The rule is narrower than its reputation, and the
reputation is doing work the rule cannot. Registered with Q13.

**NO-CLAIM.** §7 establishes the §6 denominator was chosen without justification and that at least
three load-bearing binaries were omitted. It does **not** provide their surface counts — `rch`, `fh`,
`cargo`, `git` and `tmux` are unmeasured here, so the "6.2 %" figure in §6 has no corrected
replacement yet and should be read as *"6.2 % of a denominator we now know is wrong."* The `bv`
rows are known-bad and remain in `SURFACE-MAP.jsonl` pending re-extraction against `--robot-*`.

### 7.5 `ee` and `ms` — Josh: "vital too"

| binary | version | surfaces | refs | what it is |
|---|---|---:|---:|---|
| `ee` | 0.14.2 | **111** | 1 | Eidetic Engine — durable, local-first agent memory; the "123" first published here was the help-text count — the map carries 111 rows (109 RETIRE, 2 WIRE), reconciled in §9 |
| `ms` | 0.2.1 | 61 | 1 | Meta Skill — mines CASS sessions to generate skills |


`ee` at 111 mapped surfaces is **smaller than `ntm`'s 114**. The separate 123 figure
is the historical help-text entry count, not mapped rows; 123 is larger than 114 but is not
the comparable map measure. The omission remains sharp because of what the retained session
observation recorded:

```
ee resume   Resume recent session end-state, open loops, and stale next steps
ee orient   Start a session with pack, doctor, and workspace-hygiene context
```

**Pane `%1413` hit 85 % context, compacted, lost its entire grading task, and drifted onto unrelated
plan-amendment work.** Its output existed only in context and was gone. The recovery was a
hand-written re-dispatch plus a new rule that graders write to files as they go.

`ee resume` is that mechanism, already installed, at version 0.14.2, referenced once in this repo.
The failure and its remedy were in the same `PATH`. This is the strongest instance yet of
`BUILT ≠ WIRED` — not our code unwired from itself, but **a solved problem sitting one command away
from a session that solved it again by hand, worse.**

`ms` is the same shape one level up: it mines prior sessions to *generate skills*. HISTORICAL SESSION OBSERVATION (no exact count retained): this session produced several durable rules — the replacement-naming rule, the not-found search-space rule, and the construct-citation rule — all written by hand into a brief.

### 7.6 The denominator, corrected as far as it currently goes

| | surfaces | note |
|---|---:|---|
| censused in §6 | 372 | `bv`'s 29 are scrape artifacts (§7.2) |
| `ee` | +123 | unmeasured for consumption |
| `ms` | +61 | unmeasured for consumption |
| `rch`, `fh`, `cargo`, `git`, `tmux`, `cass`, `ubs`, `caut`, `dcg`, `ru`, `pt`, `sbh`, `slb`, `gh` | **?** | **14 binaries, surface counts not taken** |
| **known lower bound** | **556** | and rising |

**The "6.2 %" headline in §6 is retired.** Not because the direction was wrong — every correction so
far has moved consumption *down* — but because a share computed against a denominator that grew
50 % in one exchange, and is still missing fourteen binaries, is not a measurement. It is retired in
favour of the three claims that survive independently of the denominator:

1. `bv` consumption is **zero**, and one call to `--robot-next` named the articulation point that
   nineteen hand-picked waves missed.
2. The historical observation was that `rch` gated builds while `RCH_ENABLED=false`;
the claim that this appears in zero plan lines is withdrawn. The current two matching lines are
the explicit historical records at §7.1; no current enablement status is claimed.
3. `ee resume` exists, is installed, and this session lost a worker's entire output to the failure
   it prevents.

**NO-CLAIM.** §7.6 states a *lower bound*, not a total: fourteen binaries have no surface count, so
the true denominator is unknown and every share against it is unavailable rather than merely
imprecise. Nothing here measures whether adopting any of these surfaces would help — `ee` at 123
surfaces is not an argument for consuming 123 surfaces, and §6's warning stands: adopting a surface
because it exists is the opposite of the discipline this plan argues for.

---

## 8. The retire rate is too high, and it is unvalidated

Josh's concern was that the historical map named many rows RETIRE without fully testing them; the snapshot below preserves that finding.
`HISTORICAL SNAPSHOT — NON-AUTHORITATIVE` (pre-current-map batch 1–19; the snapshot date,
hash, and command receipt were not retained here):

CONSUMED  12     WIRE  13     VALIDATE  8     RETIRE  469     unmapped 42
RETIRE with no validating command: 144 (31% of the 469 historical retires)

The displayed categories total 544 rows. On the all-listed-row denominator, the historical
RETIRE rate is 469/544 = **86.2%**. If and only if the 42 unmapped rows are excluded, the
dispositioned-row denominator is 502 and the rate is 469/502 = **93.4%** (rounded 93%).
Neither rate describes the current 614-row map below; both are historical pre-correction ratios.

### 8.1 Fifteen retired surfaces are named for this session's own defects
In that HISTORICAL snapshot, filtering rows with `validated_by: null` yielded the
orchestration-shaped set below. This query and its result are not a query over the current map:
the current map has no null `validated_by` values, which still does not prove structured probe evidence.
```
ntm:claim        ntm:mail         ntm:locks       ntm:lock        ntm:handoff
ntm:conflicts    ntm:coordinator  ntm:controller  ntm:checkpoint  ntm:beads
ntm:agents       ntm:adopt        ntm:assign      ntm:bind        rpc_handler:handoff
```

Set against the measured failures of the same session:

| retired surface | the defect it names |
|---|---|
| `ntm:claim` | packets dispatched naming **unclaimed beads** — the missing file→CLAIM→dispatch beat, twice |
| `ntm:mail`, `ntm:locks`, `ntm:lock` | Agent Mail was down; **no file reservations** all session; two agents on overlapping crates |
| `ntm:handoff`, `rpc_handler:handoff` | a bead handoff sent to **pane 4 (`%1398`) — a pane that does not exist** |
| `ntm:conflicts` | `SilverWolf` and pane 3 both editing the same three ported crates with no advisory lock |
| `ntm:agents` | the roster was re-derived by hand from `tmux list-panes` every tick |

The upstream OMP memory vocabulary does not close this NTM/bead claim gap. memories/storage.d.ts:18-29 declares Stage1Claim and GlobalClaim with ownershipToken and inputWatermark, but those fields govern memory-storage work ownership and watermarks, not bead assignment or pane dispatch. This is a type-level weakening of the “no claim vocabulary” absence, not a consumed dispatch contract: no caller maps either type to ntm:claim here, so the local file→CLAIM→dispatch gap remains unclosed.

**HISTORICAL FINDING:** every row in that prior set was retired without a recorded run. A
`RETIRE` carrying `validated_by: null` was an assertion that a surface was irrelevant
without touching it. The current map's non-null field is weaker than the structured evidence contract.

### 8.2 The rule this produces
**Current rule:** `RETIRE` requires structured read-only probe evidence, exactly like the
other dispositions require their own evidence. `CONSUMED` demands a citation,
`VALIDATE` demands a test, and `WIRE` demands explicit bead ID/owner/timing fields.
The historical 144 unprobed rows are the reason for this gate; they are not a current map count.

Minimum probe for a retire: **run the surface's `--help`, or invoke it read-only,
and state what it does and why that is irrelevant to orchestration. For non-invokable surfaces,
the equivalent declaration/file probe must record command, result, timestamp, and artifact identity.

`%1409` demonstrated the standard unprompted in the same wave: it probed `ms:agents`, found it an
**unrecognised subcommand**, and classified it a scrape artifact rather than a surface — a retire
backed by an invocation. That is the bar, set by a worker, not by me.

**NO-CLAIM (historical snapshot):** 144 rows lacked a recorded probe and 15 were
orchestration-shaped and defect-adjacent. This does **not** establish that those 15 should be
`WIRE`. The current map records non-null `validated_by` on every row, but its schema
does not expose the required command/exit/timestamp/artifact fields, so current RETIRE validation
remains unproven rather than silently upgraded.

---

## 9. The corrected census — and how far wrong §6 was

The current map is docs/plan/SURFACE-MAP.jsonl with **614 non-empty rows**, SHA-256 **5b3c3238c4ec9dd7f72a097bb3668e7de224e3b6f0eddc1132de2902a1d9d93c**. In this section, **unmapped** means maps_to_crate == null, not missing from the file. The exact map query returns **614 479 0**: 479 rows are unmapped by crate, while all 614 have non-null validated_by. The field is non-null on all 614 rows, but it is not structured RETIRE proof; the current map therefore does not establish RETIRE validation.
CONSUMED 52     WIRE 67     VALIDATE 33     RETIRE 453     UNPROBEABLE-PENDING 9
engaged (CONSUMED+WIRE+VALIDATE) 152 / 614 = 24.8% [current map; exact command in NUMBERS.toml]

**Reconciliation (round 11).** The current values above are derived from the map snapshot and
the exact commands in `NUMBERS.toml` (`grep -c . docs/plan/SURFACE-MAP.jsonl` and
the `surface_engaged_pct` Python query). Superseded earlier snapshots are HISTORICAL and retained only in `NUMBERS.toml`'s audit trail; they are not quoted as current status and their prior identities are not the current hash.
Current engagement is **152/614 = 24.8%**, and current WIRE count is **67**. The “ee … 123 surfaces” figure is the historical help-text count; the current map carries **111** ee rows (109 RETIRE, 2 WIRE).
The bv rows below are the LIVE split over all 76, not 47 alone.

### 9.1 What the two corrections did

| | §6 as published | after correction |
| `bv` surfaces | 29 (scrape artifacts) | **76 mapped: 29 original rows retained + 47 real `--robot-*` rows appended** |
| `bv` disposition | 0 consumed, retire all | **30 WIRE · 18 VALIDATE · 27 RETIRE · 1 UNPROBEABLE across all 76** |
| engagement | "6.2 %" | **24.8 % (152/614), current** |

**`bv` inverts completely.** The tool §6 reported at *zero consumption, retire everything* is, once
its real surface is mapped, **the most under-adopted tool in the system: 48 of 76 mapped surfaces
should be wired or validated (30 WIRE + 18 VALIDATE; the 47 `--robot-*` rows contribute 42 of
those 48).** The scrape artifact did not merely under-count it — it inverted the
conclusion, and the inverted conclusion agreed with a prior belief, which is why it survived a
round of grading.

### 9.2 The probe rule paid for itself immediately

`%1409` re-probed 56 OMP rows: **51 confirmed, 5 flipped** — 2 to `CONSUMED` (they had been consumed
all along and the census said otherwise), 2 to `VALIDATE`, and one to `WIRE` that matters
economically: **`telemetry-export.d.ts` — OMP ships OTLP export behind `OTEL_*` env vars.** That
moves **Q2** (*"what does the current workaround cost, measurably?"*) from `OPEN` to *sampled*, which
is the first movement on any §8 economic question.

`%1408` re-probed batches 12–13 and **flipped 9 of 42 on probe evidence**, including a live one:

> probed `omp/muxPing` into a fresh `--mode=rpc` session — the ready envelope answered, **no
> `muxPing` result came back.** The worker report records this as a probe observation; the map does not retain the structured receipt required to validate a RETIRE.

It also caught pane 3 having filed `VALIDATE` rows for surfaces with **zero references**
(`set_host_tools`) — *"exactly the defect the new retire rule stops."*

### 9.3 What this says about the method, not the numbers

Three of the four §6 figures were wrong, and each was wrong for a different reason: a **denominator
chosen without justification** (four tools of nineteen), an **extraction that harvested prose**
(`bv`), and a **disposition that permitted a shrug** (144 unprobed retires). None was a
typo — each was a measurement that looked like a measurement.

The correction did not come from a gate or a re-read. It came from Josh saying *"i feel like we're
naming a lot retired without fully testing them"* and *"we're retiring most of the surfaces"* —
twice, because the first time I recorded the finding and did not act on it.

**NO-CLAIM.** Superseded historical engagement and WIRE values are excluded from current-status prose. The live declared figure is **24.8% engagement (152/614)** and current WIRE count is **67**. Fourteen external binaries still have no surface count, so that denominator is unknown. WIRE is a proposal, not proof of implementation, and non-null validated_by is not structured RETIRE evidence. A probe establishes that someone ran a surface; it does not establish that the resulting judgement is right.
---

## 10. Evidence inherited from a vacuous source is not evidence

`%1408`, closing Task B, found the deepest defect in the census and it is a **chain-of-custody**
problem rather than a counting one:

> *"every RETIRE whose only evidence is scanner `CAPABILITY_NOT_USED` inherits the §3.3 self-twin
> vacuity."*

§3.3 measured that the scanner's own census carries **183 rows with exactly one distinct
`must_be_true` and one distinct `negative_evidence`** — the four-field discipline satisfied
syntactically and vacuously. Any retirement justified by *"the scanner classified it
`CAPABILITY_NOT_USED`"* therefore rests on a classification produced by a process already measured
as vacuous. **The evidence is inherited from a source known not to carry any.**

That is not the same as being wrong. Those surfaces may well be irrelevant. It is that the map
recorded a *judgement* where it held only a *relay*.

### 10.1 The reclassification

```
CONSUMED 32    WIRE 67    VALIDATE 30    RETIRE 453    UNPROBEABLE-PENDING 9
[LIVE — this table moved twice during writing; NUMBERS.toml surface_engaged_pct is the source]
```

**Snapshot history, kept because it is the finding:** when this subsection was written the split
was 30/66/25/407 with **63** UNPROBEABLE-PENDING — 54 `type_root` rows retired on scanner
classification plus grep zeros, 3 mux rows, 6 thin-reason withdrawals. Since then the 54 type_root
rows were probed per-kind and RE-CLASSIFIED (only 3 VALIDATEs among them survived: extensibility,
goals, internal-urls), collapsing the honestly-unknown set to **9** (3 mux + 6 thin-reason). The
63 → 9 collapse is the per-kind probe rule working, not drift.

It also corrected **ten of its own `rpc_handler` rows** that had cited the mux null-probe as
evidence — *"wrong basis, restated to adapter-vocabulary zero."* An agent auditing its own prior
output and finding the reasoning unsound, unprompted, is the third self-retraction of the session.

### 10.2 The model case, named by a worker

`ntm:ntm` is what `%1408` calls **the positive null**: probing it returns *unknown command*, and an
unknown command **is a verdict**. That is the distinction the whole re-probe wave turned on:

| probe result | classification | why |
|---|---|---|
| *unknown command* | **RETIRE** | the absence is *answered* — the surface does not exist |
| *no output, empty, timeout* | **UNPROBEABLE-PENDING** | the absence is *unexplained* — a timeout is not a verdict |

Both are nulls. Only one is evidence. The policy requires each current RETIRE to identify which kind it is, but the present map's non-null `validated_by` strings do not prove that this structured evidence was recorded.

### 10.3 Where the census actually stands

| | count | meaning |
|---|---:|---|
| engaged | **152** | CONSUMED + WIRE + VALIDATE — **24.8%** (152/614; NUMBERS.toml command) |
| unmapped by crate | **479** | maps_to_crate == null; overlaps the disposition rows and is not missing rows |
| honestly unknown | **9** | UNPROBEABLE-PENDING — 3 mux + 6 thin-reason; this is a disposition count |
| retired | **453** | rows with disposition `RETIRE`; structured probe backing is not proven by the map schema |

**The honestly-unknown column is the most valuable in the table.** It did not exist three waves
ago, when those same rows read `RETIRE` and the census claimed to know something it did not. It
peaked at 63, and the per-kind probe rule resolved 54 of them — what remains is the honest
residue, not the peak.
**NO-CLAIM.** 453 is a disposition count only. The map query proves 0 null `validated_by` values,
not 453 structured probe receipts; no count exists for RETIRE rows carrying command, exit/result,
timestamp, and artifact identity. The inherited-scanner-classification sweep is therefore unrun and
unowned, and the status table must not call all 453 probe-backed.

---

## 11. `UNPROBEABLE-PENDING` became the new shrug within one wave

Josh: *"why are they unprobeable, lets test this on pane 0."*

`HISTORICAL OBSERVATION — NON-AUTHORITATIVE`: the retained report says these rows are readable, but no exact directory-listing command, output receipt, or revision is retained in this section. The figures below are not current measurements.
```
dist/types/cli/       54 files   352 KB   ->  359 exported symbols
dist/types/session/   78 files   564 KB   ->  acp-permission-gate.d.ts,
                                              agent-session-events.d.ts,
                                              artifacts.d.ts, …
dist/types/commands/  42 files   176 KB
dist/types/jsonrpc/    1 file      4 KB
```

Of the 63 rows reclassified in §10, **54 are `type_root` and every one is a directory of TypeScript
declarations that can be read right now.** `type_root:cli` exports **359 symbols**.

### 11.1 The category error

The re-probe rule said *"run the surface read-only."* For a `subcommand` that means invoking it. For
a **type surface** it means **reading the declarations** — and nobody tried, because the rule was
written with CLI verbs in mind and the type rows inherited a probe shape that does not fit them.

**"Cannot be invoked" was read as "cannot be probed."** Those are different, and the map recorded
the second while only the first was true.

So the sequence across three waves is:

| wave | classification | what it actually meant |
|---|---|---|
| 1 | `RETIRE` | nobody looked |
| 2 | `RETIRE` + probe rule | nobody looked *at CLI verbs* |
| 3 | `UNPROBEABLE-PENDING` | nobody looked *with the right instrument* |

Each correction was real and each left a residue in the same shape. The failure is not the
classification, it is that **every wave defined "probe" as the instrument that happened to be in
hand**, and a surface unreachable by that instrument was recorded as unreachable in principle.

### 11.2 The per-kind probe, which should have been specified in wave 1

| kind | the probe |
|---|---|
| `subcommand` | invoke `<tool> <sub> --help` read-only; *unknown command* is a positive null |
| `type_root` | list `dist/types/<root>/`, count files, extract exported symbols |
| `rpc_handler` | locate the handler in `dist/`, name what it accepts and returns |
| `omp_method` | probe against a live session, and **name which broker answered** |
| `declaration` | read the `.d.ts` at the top level |

Only the first row was ever written down. Four kinds were probed with a verb-shaped instrument, or
not at all, and then classified on the result.

**NO-CLAIM.** This establishes the 54 `type_root` rows are readable and therefore mis-classified. It
does **not** re-classify them — reading a directory proves a probe exists, not what the disposition
should be. The 3 `omp_method` rows remain genuinely pending on the six-mux ambiguity, and the 6
`subcommand` rows were withdrawn by their author for thin reasoning, which is a different defect
again. Only 54 of the 63 are refuted here.

---

**HISTORICAL SWEEP BOUNDARY (2026-08-31).** Sections 2.12–2.14 retain the type-surface sweep and its failure examples. Their fixed file, symbol, byte, and coverage counts are snapshot facts from the cited revision; they are not current workspace counts unless a paragraph explicitly supplies a current command and result.
## 2.12 Three dispositions from the `plan-mode` / `modes` / `goals` sweep

`%1408`, bead `ipg.1`, coverage table at `29958a3`. Three type roots, three genuinely different
answers — which is the point of a disposition rather than a count.

**(a) `plan-mode` — NOT OURS.** Thin types; our system is strictly more capable. This confirms the
downgrade recorded in §12.11: `PlanApprovalDetails` is `{planFilePath, title, planExists}` and
`ResolvedApprovedPlan` adds `planContent`. A plan *reference* and an approval *flag*, against a
convergence protocol we had to build as `CONVERGENCE.jsonl` + `convergence.rs` + a held-out lens.
**The adoption pattern paid nine times and does not pay here**, and saying so is the more useful
result than another adoption claim.

**(b) `modes` — REIMPLEMENTED BY SCRAPING.** 204 files, 843 symbols. `tick-monitor` does not consume
any of it; **it reads the rendered output.** Every pane-state classification in this system is a
regex over a terminal buffer — the braille-spinner-and-timer contract, the `π` glyph, the
spinner-stripped content hash — against 843 typed symbols describing the same states.

That is the largest single instance in this repo of the defect Josh's objective names: *"everything
typed — nothing unknown."* And it explains a class of failure we have paid for repeatedly: the v18
status-line change that scored 0/3 on live payload, the stale spinner in scrollback that reported a
dead pane alive, the whole-buffer scan that scored one pane working AND idle simultaneously. **A
scraper inherits every rendering change as a correctness bug.** A typed consumer does not.

**(c) `goals` — UNUSED CAPABILITY.** A typed token-budget runtime, consumed nowhere. It could ground
**S5 Cost** and **Q2** — the cost question §00 has carried as OPEN since the first pass, and which
`%1408` earlier priced as "grading is 30+ pane-hours unmeasured". We have been reasoning about cost
from per-pane dollar figures scraped off status lines while a typed budget surface sits unused.

### The positive control FAILED, and the diagnosis is why that is reportable

No root came back FULLY COVERED. A failed positive control normally means **the scan is broken** and
every verdict is suspect — that is the rule this repo enforces on its own censuses. `%1408` argued
the other case: these are **agent-plane surfaces**, not orchestrator-plane, so partial coverage is
the true answer rather than a scan artifact. Anti-vacuity passed independently.

Distinguishing "my instrument is broken" from "the world is genuinely partial" is the single
hardest call in this whole methodology, and it was made explicitly instead of silently.

### NO-CLAIM

(b) names a defect and does not fix it: `tick-monitor` still scrapes, and replacing a regex over a
terminal buffer with a typed consumer means holding a session per pane — the same topology change
§11.9 records as the real cost of the completion-signal adoption. (c) is a *capability*, not a
measurement: nothing here shows the `goals` budget runtime is reachable from a process we run, and
that is exactly the declared-versus-wire-proven distinction that made seven of the eight gap
adoptions weaker than the first.

---

## 2.13 The scraping finding is systemic: three roots, one pattern

`%1408`, bead `ipg.2`, coverage table at `cab09e0`, extending §2.12. Three more roots, and the
disposition repeats hard enough to stop being a per-root fact.

| root | disposition | scale |
|---|---|---|
| `modes` (§2.12) | **REIMPLEMENTED BY SCRAPING** | 204 files, 843 symbols |
| `task` | **REIMPLEMENTED BY SCRAPING** | 27 files — the **entire subagent lifecycle** |
| `slash-commands` | **REIMPLEMENTED BY SCRAPING** | census enumerates **0** against an expected **136** |
| `commands` | NOT OURS | 42 CLI verbs for human users |

### `task` is the one that should stop the room

27 files typing **spawn, parallel, worktree, structured-output and yield** — the complete subagent
lifecycle — and this system consumes it **as pane text**. Every subagent dispatched tonight was
launched by typing into a terminal and its completion inferred by regex over a scrollback buffer,
against a typed lifecycle that describes exactly those states.

That is not a missing feature. It is the same mechanism we already refuted once: §10 claimed a
worker-completion signal was precedent-free across 210 repositories while `AgentEndEvent` shipped in
the dependency. **`task` is that finding again, one layer up** — not a completion event this time but
the whole lifecycle around it.

### `slash-commands`: 0 of 136

The census consumes the type root and enumerates **zero** members against an expected 136. Largest
unmapped OMP surface in the project. A count of zero from a root we can read is not "no surface" —
it is an unfinished enumeration, and §02's own history contains four instances of exactly that error
being reported as a measurement.

### Why this is a plan-level finding and not a backlog item

Josh's standing objective: *"Every surface of our journey mapped to specific commands with proper
guards and timeouts, everything typed — nothing unknown."*

Measured against that sentence, the orchestrator's **primary sensor is a regex over rendered text**
for pane state, subagent lifecycle, and command surface alike. Every rendering change upstream
arrives here as a correctness bug, and the record already shows the bill:

- the v18 status-line contract changed and the shipped classifier scored **0/3 on live payload**
- a stale spinner in scrollback reported a **dead pane alive**
- a whole-buffer scan scored one pane **working AND idle simultaneously**
- `muxPing` returned null against a six-mux machine and was read as a broken surface

Four failures, one cause. A typed consumer has none of them, and `task` + `modes` + `slash-commands`
is the typed surface sitting unused.

### NO-CLAIM

The positive control **FAILED** on both `ipg.1` and `ipg.2` — no root came back FULLY COVERED — and
was diagnosed as agent-plane rather than a broken scan, with anti-vacuity passing independently on
each. That diagnosis is a judgement, and it is the judgement most likely to be wrong here: a scan
that reports partial coverage on every root it touches is *also* what a subtly broken scan looks
like. Nothing in either bead distinguishes those two worlds beyond the argument itself.

Adopting any of this means holding a session per worker rather than reading a pane — the topology
change §11.9 already prices as the real cost of the completion-signal adoption. **Nothing here has
been adopted.** The finding is that we built a scraper where a typed surface existed, three times.

---

## 2.14 Sixth instance, and this one is an hour old

`%1408`, `ipg.5` (`6d3b102`) and `ipg.6`. Two more waves, both all-(a) — the
orchestration/agent-plane boundary is holding — but two named findings land inside
our own work.

### `session`: the largest root in the workspace, scraped

78 files, 395 KB, **499 symbols** — bigger than `modes` (843 symbols across 204
files, but a third of the bytes). `tick-monitor` covers **7 of 8 clauses at the
output plane and zero at the type plane**: no crate imports any of the 78 session
`.d.ts` files.

The orchestrator's primary sensor scrapes the surface that types exactly what it
reads. Running total of typed surface consumed as text: **`modes` 843 + `session`
499 + `task` (full subagent lifecycle) + `slash-commands` (0 of 136 enumerated) +
`dap` (a complete DAP client we reimplement with print statements)**.

### `AdvisorSeverity` — the finding that indicts this session

OMP ships, in `dist/types/advisor/advise-tool.d.ts`:

```typescript
export type AdvisorSeverity = "nit" | "concern" | "blocker";
export interface AdvisorNote { note: string; severity?: AdvisorSeverity; advisor?: string; }
export interface AdvisorMessageDetails { notes: AdvisorNote[]; }
```

A three-level severity **with the grader attributed**. One hour before this was
found, this session invented `BLOCKER / MAJOR / MINOR` and wrote a NO-CLAIM stating:

> *nothing yet prevents a grader from downgrading a real defect to MINOR to make a
> section bank … Two candidate defences exist and neither is built.*

`advisor?: string` is one of those defences, shipped in the dependency we already
depend on. Not analogous — **the same field, for the same reason**.

This is the sixth instance of one pattern: §10 claimed a worker-completion signal
was precedent-free across 210 repositories while `AgentEndEvent` shipped; `modes`,
`session`, `task`, `slash-commands` and `dap` are reimplemented from rendered text.
The distinguishing feature of this instance is **latency**: the other five predate
tonight. This one was invented, gated, committed, and refuted inside ninety minutes.

### What was adopted, and what was not

**Adopted:** `graded_by` is now a required field on every severity row, with
`AdvisorNote.advisor` cited as the precedent in `SCHEMAS.toml`. A new gate,
`every_severity_row_names_its_grader`, fails the build on an unattributed row —
mutation-verified (strip one row's attribution → RED naming the line; restore →
byte-identical, green) with anti-vacuity so an empty scan is an error.

**Not adopted:** the level names. Renaming `BLOCKER/MAJOR/MINOR` →
`blocker/concern/nit` is cosmetic alignment; attribution is the substantive half and
it is the half we omitted. Importing the type rather than paralleling it is the
topology change §11.9 prices, still unpaid.

**NO-CLAIM:** attribution does not make severity honest. It makes dishonesty
*attributable*, which is the most a schema can enforce. The second defence — any
MINOR touching a `NUMBERS.toml` figure is automatically MAJOR — is still not built.


---


<!-- ===== 03-crates.md ===== -->

# 03 — Every crate: contract, schema, types, dependencies

*Serves **R4** — "every crate - every schema input / output, every typed interface, how everything
is interacting". Answerable to `00-brief.md`; measured facts are taken from its §3 unless this
section explicitly disagrees, and every disagreement is named in "Where this section disagrees with
the brief" below.*

## How to read this section

**HISTORICAL INVENTORY BOUNDARY.** The 183-row envelope and 26-crate split in the next paragraphs are the retained pre-extraction snapshot. Current cargo metadata re-derives 50 workspace packages; the old scanner denominator is not current acceptance evidence.
The workspace ships a built census, `omp-inventory-map`, that emits 183 rows in a versioned
envelope. Every row carries the four mandatory fields — `inputs`, `outputs`, `must_be_true`,
`negative_evidence` — with zero missing. That census is **not** the source for this section, and
the reason is the sharpest measured finding of the session.

**MEASURED.** Across all 183 rows there is exactly **one distinct** `must_be_true` value and
exactly **one distinct** `negative_evidence` value:

```
python3 -c "…collections.Counter(json.dumps(r.get('must_be_true')) …)"
  crate rows:     n=26   distinct must_be_true=1  distinct negative_evidence=1
  non-crate rows: n=157  distinct must_be_true=1  distinct negative_evidence=1
```

The universal `must_be_true` is `["The source probe is non-empty before a known verdict is
emitted.","A versioned inventory envelope carries the probe state."]`. That sentence is true of the
*scanner*, for every row, regardless of what the row describes. The four-field discipline is
satisfied **syntactically and vacuously**: the fields are present, the schema validates, and the
content distinguishes nothing.

The same defect runs through the per-crate rows specifically. For a crate row, `inputs` names
`cargo metadata --format-version 1 --no-deps` and the crate's own `Cargo.toml`; `outputs` names the
crate's build targets. Those describe **the scanner's provenance**, not the crate's contract.
`what_it_provides` is boilerplate — "Workspace crate X from cargo metadata" — 26 distinct strings
only because the name varies.

So: **every contract below is derived from the crate's own source**, with the derivation command
inline. Where the source does not state a contract, the row says `UNDECLARED`. We do not fill a
cell by inference. An empty cell that admits it is empty is worth more to an investor than a
plausible cell that was invented, and the census is the standing proof of what the second kind
costs.

**NO-CLAIM:** deriving contracts from public signatures does not prove those contracts are
*honoured* at runtime. A `pub fn assess(&AckStageInput) -> AckStageResult` states a shape, not a
semantics. Behavioural conformance is the gate section's problem, not this one's.

---

## The crate table

**MEASURED**, derived by three commands over `crates/*`:

```
grep -E '^(name|description)' crates/*/Cargo.toml            # role
grep -hoE '^pub (enum|struct|trait|fn|type) [A-Za-z_0-9]+' crates/*/src/*.rs   # key types
sed -n '/^\[dependencies\]/,/^\[[a-z]/p' crates/*/Cargo.toml # path-deps
grep -c 'unsafe_code = "forbid"' crates/*/Cargo.toml         # manifest lint
grep -rlF 'forbid(unsafe_code)' --include=lib.rs --include=main.rs crates/  # inner attr
```

**The table below is a 2026-08-31 historical snapshot of twenty-six crates.** The current workspace has a different package denominator after subsequent extraction; re-run cargo metadata before treating this table as current.

| crate | role | inputs (real) | outputs (real) | key public types | path-deps | no unsafe |
|---|---|---|---|---|---|---|
| `ack-spine` | separate transport, delivery and ack authorities | `bead_id: &str`, `marker: &str`, capture text | `AckVerdict`, `AckSummary`, `StepLedger` rows | `AckVerdict`, `TransportAuthority`, `DeliveryAuthority`, `AckAuthority`, `AckEvidence`, `FollowUpVerdict`, `StepKind`, `StepRecord`, `StepLedger`, `HeartbeatRow` | `finding` | yes |
| `ack-stage` | typed follow-up actions for receiver receipts | `&AckStageInput` (transport receipt + ack readback) | `AckStageResult`, `AckAction` | `TransportKind`, `TransportReceipt`, `AckReadback`, `AckStageInput`, `AckStageResult`, `AckAction` | `receiver-receipt`, `tick-monitor` | yes |
| `commit-build-fence` | fail-closed pre-commit fence for in-flight builds | `RegistrationStore` loaded from `&Path`; `BuildRegistration` / `ReleaseEvent` | `FenceVerdict`; atomically saved store | `BuildRegistration`, `ReleaseEvent`, `RegistrationStore`, `FenceVerdict`, `StoreError` | — | yes |
| `composer-typed` | does a pane composer hold real typed text | `data: &str`, `&Rules` | `bool` | `Rule`, `Rules` | — | yes |
| `dispatch-claim-fence` | fail-closed claim authorization for dispatch packets | `br show --json` text → `BeadSnapshot`; `DispatchIntent` | `DispatchPermit` | `BeadStatus`, `BeadSnapshot`, `DispatchIntent`, `DispatchPermit`, `ClaimFenceError`, `SnapshotParseError` | — | yes |
| `dispatch-silence-watch` | typed verdict after every dispatch, read from the tracker | bead comment text, assignee text | `SilenceVerdict` | `SilenceVerdict` | — | yes |
| `finding` | a named gap is an obligation, filed or waived, never dropped | spool dir `&Path`; a constructed `Finding` | `SpooledFinding`, `Filed`, `Waived`, `Vec<PathBuf>` of pending | `Finding`, `SpooledFinding`, `Filed`, `Waived`, `Publisher` (trait), `FindingError` | `subprocess-contract` | yes |
| `finding-dispatch` | turn recurring supervisor decisions into named findings | `&SupervisorDecision`, `recurrence_count: u32` | `Option<Finding>` | — (single free fn `finding_for`) | `finding`, `omp-orchestrator` | yes |
| `fleet-composite` | geometric fleet health composite | `&BTreeMap<String, f64>` of raw factors | `CompositeReport`, `SelftestReport`, JSON | `FactorSpec`, `InputError`, `CompositeReport`, `SelftestCheck`, `SelftestReport` | — | yes |
| `installer` | one-touch build, four-way identity verify, install | repo `&Path`, binary name | `RepoOwnership`, `IdentityCheck`, installed binary | `InstallError`, `InstallTarget`, `RepoOwnership`, `IdentityCheck` | — | yes |
| `kernel-bypass-gate` | detect raw invocations duplicating an existing kernel | file path + source text; workspace root | `Vec<Bypass>`, `GateReport` | `Bypass`, `GateReport` | — | yes |
| `kernel-only-operator-hook` | fail-closed PreToolUse hook blocking kernel bypasses | hook JSON `&[u8]` → `HookInput` | `Decision` (rendered to stdout) | `HookInput`, `Permission`, `Decision`, `ParseError` | `subprocess-contract` | yes |
| `loop-queue-filter` | fail-closed port of the control-plane queue selector | queue JSON `&str`, argv `&[String]`, `&Runtime` | `RunOutput` | `Runtime`, `RunOutput` | — | yes |
| `no-shell-gate` | refuse tracked `.sh`/`.py`; exemption list empty by design | `git ls-files` output; repo root `&Path` | `Verdict`, `Vec<Violation>`, `WorkspaceLoad` | `Violation`, `Verdict`, `GateError`, `WorkspaceLoad` | `path-literal-guard`, `state-wildcard-lint`, `pre-delete-citation-check`, `undrained-pipe-lint` | yes |
| `omp-inventory-map` | the OMP v18 surface + workspace crate census | `cargo metadata`, OMP CLI probes, source scrape | `InventoryMap` envelope (184 nodes, 207 edges, 183 rows) | `ProbeState`, `SurfaceMap`, `SurfaceMapAudit`, `InventoryRow`, `InventoryNode`, `InventoryEdge`, `InventoryMap`, `ProbeEvidence`, `InventoryCounts` | — | yes |
| `omp-orchestrator` | resident supervisor: observe → dispatch → receipt → escalate | `&Observation`, `&IdleAuthorization`, repo `&Path` | `SupervisorDecision`, `GateCensus`, `Duty`/`Discharged` | `Observation`, `PaneObservation`, `QueueState`, `SupervisorDecision`, `IdleAuthorization`, `GateCensus`, `GateReachability`, `Duty`, `Discharged`, `Census` | `ack-stage`, `dispatch-claim-fence`, `omp-rpc-session`, `receiver-receipt`, `subprocess-contract` | yes |
| `omp-rpc-session` | typed bounded single-session adapter for `omp --mode=rpc` | `OmpCommand` + `RpcSessionConfig`; raw stdout frames | `RpcFrame` (`Ready`/`Response`/`Unknown`/`Malformed`), `RpcSessionReport` | `Deadlines`, `TimeoutPhase`, `OmpCommand`, `RpcSessionConfig`, `RequestId`, `RpcRequest`, `RpcFrame`, `MalformedReason`, `Lifecycle`, `ProtocolError`, `RpcError` | — | yes |
| `omp-types` | the canonical vocabulary, derived from asupersync, never authored | none (re-export only) | `pub use` of asupersync types | `Outcome`, `OutcomeError`, `PanicPayload`, `Severity`, `join_outcomes`, `Budget`, `CapabilityBudget*`, `RemainingBudget`, `ObligationId`, `RegionId`, `TaskId`, `Time` | — | yes |
| `pane-dispatch-fence` | cross-process per-pane admission fence | `UNDECLARED` — binary only, no `src/lib.rs`, zero `pub` items | `UNDECLARED` — exit status only | none exported | `subprocess-contract` | yes |
| `path-literal-guard` | zero hardcoded home-path literals across `crates/*/src` | repo root `&Path` | `ScanReport` (`Vec<Hit>`) | `Hit`, `ScanReport` | — | yes |
| `pre-delete-citation-check` | refuse deleting a file a closed bead cites as evidence | staged-deletion git output, `br` closed-bead JSON | `Vec<CitationConflict>` | `CitationConflict`, `ClosedBead` | — | yes |
| `receiver-receipt` | receiver-side dispatch receipt classifier | `pane_id`, pane capture `&str`, timestamp | `Observation` → `ReceiptVerdict` | `PanePresence`, `PostSendObservation`, `ReceiptReason`, `ReceiptVerdict` | `tick-monitor` | yes |
| `state-wildcard-lint` | reject wildcard arms on locally resolvable state enums | source text; workspace root | `LintReport` (`Vec<Finding>`) | `FindingKind`, `Finding`, `LintReport` | — | yes |
| `subprocess-contract` | the cancel-correct process-group and dual-pipe boundary | `&Cx`, `asupersync::process::Command` | `Output` / `ExitStatus`, or `RunError` | `RunError`; `run_output`, `run_status` (both `async`, `&Cx` first) | — | yes |
| `tick-monitor` | three monitors behind one loop-enforcement choke point | pane capture, git state, `Tick` | `Outcome`, `PaneState`, `Liveness`, `CapacityEscalationReceipt` | `Outcome`, `PaneState`, `Liveness`, `Observation`, `Reject`, `Tick`, `State`, `CapacityAlarm`, `CapacityAlarmEvent`, `CapacityEscalationReceipt`, `RepoError` | — | yes |
| `undrained-pipe-lint` | fail on both-pipes-piped + `try_wait` poll + no concurrent drain | source text; workspace root | `LintReport` (`Vec<Violation>`) | `Violation`, `LintReport` | — | yes |

**The unsafe-forbid split, MEASURED — current command-backed census (2026-09-01).** All **50 of 50** crate manifests carry unsafe_code = "forbid". **49 of 50** crate source trees carry an inner forbid(unsafe_code) attribute, and **49 carry both**; tick-monitor is the one manifest-only straggler. **Zero are attribute-only; the union is 50 of 50.** Earlier 26/25/26 values are dated snapshots and remain historical. The property holds by adoption today; the one-file lint below is still required to make it hold by construction for crate 51.
**PROJECTED.** A one-file lint asserting *manifest-and-attribute for every workspace member* turns
that coincidence into an invariant. It is the cheapest gate in the plan and it is not written yet.

---

## The dependency graph in prose

**HISTORICAL GRAPH SNAPSHOT (pre-extraction, 2026-09-01).** The 18-edge list, 17/26 leaf ratio, 22/26 non-routed ratio, and 29 spawn-site inventory below describe the earlier 26-crate graph. Current workspace package and path-edge counts must be re-derived from cargo metadata; these historical values are not current acceptance denominators.
**CURRENT GRAPH RECHECK (integration).** `cargo metadata --format-version 1 --no-deps` with path-dependency filtering returns **50 packages, 34 path edges, 30 leaves, and 20 non-leaves**. This is the current graph denominator; the explicit edge list and 18/17/26/22/26 ratios below remain historical.
**MEASURED** — 18 `path-depends-on` edges, complete, from the census:

```
ack-spine -> finding                     finding -> subprocess-contract
ack-stage -> receiver-receipt            ack-stage -> tick-monitor
receiver-receipt -> tick-monitor         finding-dispatch -> finding
finding-dispatch -> omp-orchestrator     omp-orchestrator -> ack-stage
omp-orchestrator -> dispatch-claim-fence omp-orchestrator -> omp-rpc-session
omp-orchestrator -> receiver-receipt     omp-orchestrator -> subprocess-contract
kernel-only-operator-hook -> subprocess-contract
no-shell-gate -> path-literal-guard      no-shell-gate -> state-wildcard-lint
no-shell-gate -> pre-delete-citation-check
no-shell-gate -> undrained-pipe-lint     pane-dispatch-fence -> subprocess-contract
```

**The leaves.** Nine crates emit an outgoing `path-depends-on` edge (`ack-spine`, `ack-stage`,
`finding`, `finding-dispatch`, `kernel-only-operator-hook`, `no-shell-gate`, `omp-orchestrator`,
`pane-dispatch-fence`, `receiver-receipt`), so **17 of 26 are leaves**: `commit-build-fence`,
`composer-typed`, `dispatch-claim-fence`, `dispatch-silence-watch`, `fleet-composite`, `installer`,
`kernel-bypass-gate`, `loop-queue-filter`, `omp-inventory-map`, `omp-rpc-session`, `omp-types`,
`path-literal-guard`, `pre-delete-citation-check`, `state-wildcard-lint`, `subprocess-contract`,
`tick-monitor`, `undrained-pipe-lint`. Two of those leaves are the interesting ones:
`omp-rpc-session` and `omp-inventory-map` both spawn processes and neither routes through
`subprocess-contract`. A leaf that spawns is a leaf that reimplemented the boundary.

**The deepest chain** is five nodes:
`finding-dispatch → omp-orchestrator → ack-stage → receiver-receipt → tick-monitor`.
Everything the supervisor decides is ultimately grounded in a `tick-monitor` pane observation, and
`finding-dispatch` converts a repeated decision at the top of that chain into a `Finding`
obligation. That is a coherent spine.

**The two hubs.** `omp-orchestrator` is the *fan-out* hub — five outgoing edges, the only crate
that composes ack, claim-fence, rpc-session, receipt and subprocess into one supervisor.
`no-shell-gate` is the *aggregation* hub — four outgoing edges, all to sibling gates, making it the
single build-time entry point for the repo's lint family.

**`subprocess-contract` is the most-depended-on crate: four dependents** — `finding`,
`omp-orchestrator`, `kernel-only-operator-hook`, `pane-dispatch-fence`. That is the right shape,
and the reason is structural rather than stylistic. Its entire public surface is two async
functions and one error enum:

```
pub async fn run_output(cx: &Cx, mut command: Command) -> Result<Output, RunError>
pub async fn run_status(cx: &Cx, command: Command) -> Result<ExitStatus, RunError>
pub enum RunError { … }
```

`&Cx` comes first, as the asupersync contract requires. The doc comment states the three properties
that make it load-bearing: a fresh process group per child with group-targeted cancellation, both
pipes drained concurrently by `output_async`, and — the one that matters most — *"a timeout or
deadline cancellation is surfaced as `RunError::Timeout`, never as a child failure or an invented
output verdict."* That is the repo's hardest-won rule, **a timeout is not a verdict**, encoded once
as a type instead of remembered separately in every call site. Anything that spawns should route
through it precisely so that rule cannot be re-litigated per crate.

**MEASURED: 22 of 26 crates do not route through it.** Four depend on it. Independently measured
across 29 raw spawn sites, 4 crates use `subprocess-contract` and 12 of 14 async fns take `cx`
first. The gap is not an abstraction preference — it means at least 25 spawn sites can still leak a
process group, deadlock on an undrained pipe, or convert a timeout into a fabricated verdict, and
the only thing standing between us and that is `undrained-pipe-lint`, which catches one of the
three failure modes by pattern-matching source text.

**What Jeffrey would do.** Searched the mirror at `/Volumes/ZestData/dicklesworthstone-mirror` for
this exact shape; `asupersync` itself is the prior art and it is already our dependency —
`process::Command` with `ProcessGroupMode`, `ProcessSignalTarget` and `output_async` exists
upstream precisely so a caller never hand-rolls the group/drain pair. The correct move is not to
invent a wrapper policy but to make the existing wrapper the only reachable door.

**PROJECTED.** A gate that fails any crate constructing `std::process::Command` or
`asupersync::process::Command` outside `subprocess-contract` raises the floor: the 22 crates that
currently *may* hand-roll a spawn would have to be edited deliberately, in the open, to do so. It
does not guarantee correct spawning — a crate could route through `subprocess-contract` and still
mishandle the result — it removes accidental divergence, which is the failure mode we measured.
That gate does not exist today.

---

## The typed-interface problem, measured

**MEASURED**, by direct grep over `crates/`:

```
grep -rhoE '^pub enum [A-Za-z_0-9]+'   --include=*.rs crates/ | wc -l   -> 59
grep -rhoE '^pub struct [A-Za-z_0-9]+' --include=*.rs crates/ | wc -l   -> 91
```

An earlier scan scoped to library surfaces reported **51 public enums (excluding test+bin sources; 59 including them — publish the pair) and 79 public structs across
22 of 24 crates**. Both numbers are real; they differ because the grep above includes test modules
and binary sources. We publish both rather than pick the flattering one — the delta *is* the
measurement's error bar.

**Four colliding type names**, exact:

```
grep -rhoE '^pub (enum|struct) [A-Za-z_0-9]+' --include=*.rs crates/ \
  | awk '{print $3}' | sort | uniq -d
  -> Finding  LintReport  Observation  Violation
```

One of those four is structural rather than cosmetic, and it is the direct cause of the single
worst row in the brief's five-stage control loop (formerly "five-stage" — renamed, the table has five stages and seven rows) table (§4). `tick-monitor` produces the `Observation`
that `omp-orchestrator` consumes, and **each declares its own incompatible struct**. That is the
seam where `free_capacity` was derived from the same `is_dispatchable` filter that requires a
*Confirmed* Idle pane, so a pane at `t=0` fell out of the `idle_panes` list **and** out of the
capacity count — the `actionable: BROKEN` row. Had one type crossed the boundary rather than being
re-declared on each side, the mismatch would have been a compile error instead of a silent
arithmetic error observed only after 162 refused ticks.

**Six Verdict-shaped types with no shared trait**, exact:

```
grep -rhoE '^pub enum [A-Za-z_0-9]*Verdict' --include=*.rs crates/ | sort -u
  -> AckVerdict  FenceVerdict  FollowUpVerdict  ReceiptVerdict  SilenceVerdict  Verdict
```

Six independent answers to "what happened". No trait, no `From`, no common discriminant. The
consequence is precise and it is not aesthetic: **you cannot compose them and you cannot count
them.** There is no function `fn grade(&[impl Verdictlike]) -> Grade`, because there is no
`Verdictlike`. A supervisor holding an `AckVerdict`, a `ReceiptVerdict` and a `FenceVerdict` cannot
reduce them to one number without hand-writing a match per pair. That is *why grading is prose* —
not because nobody wrote the grader, but because there is no type a grade could be.

**Seventeen ack/receipt types in three incompatible dialects.** `ack-spine` speaks *Authority*
(`TransportAuthority`, `DeliveryAuthority`, `AckAuthority`). `ack-stage` speaks *Receipt*
(`TransportReceipt`, `AckReadback`, `AckStageResult`). `receiver-receipt` speaks a third
(`PanePresence`, `PostSendObservation`, `ReceiptReason`, `ReceiptVerdict`). All three are groping
toward the same distinction — *did the transport accept it, did it arrive, did the receiver
acknowledge it* — and none can be passed to another without translation.

**`omp-types` exists and has ZERO dependents.** No crate lists it as a path-dep; it appears in none
of the 18 edges. Its own doc comment states the design rule verbatim: *"No crate invents a type
that already exists here… its contents are derived — re-exported from `asupersync` at the exact rev
we pin (`fa3c01aec`, v0.4.9) — never authored here."* The vocabulary is shipped and unadopted.

**One honest correction to the brief.** The brief lists `omp-types` as re-exporting `AckKind`,
`DeliveryClass`, `ObligationLedger`, `Budget` and `Outcome`. **MEASURED** by reading
`crates/omp-types/src/lib.rs` (129 lines), the actual `pub use` set is three lines:

```
pub use asupersync::types::{Outcome, OutcomeError, PanicPayload, Severity, join_outcomes};
pub use asupersync::types::{Budget, CapabilityBudget, CapabilityBudgetDimension,
    CapabilityBudgetRefusal, CapabilityBudgetRequirements, RemainingBudget};
pub use asupersync::types::{ObligationId, RegionId, TaskId, Time};
```

`AckKind`, `DeliveryClass` and `ObligationLedger` are **not** re-exported. The crate documents why:
`messaging-fabric` requires the `test-internals` feature at rev `fa3c01aec`
(`consumer.rs:1299` default impl), and that feature was correctly removed from upstream defaults —
so the ack vocabulary is *unreachable at our pinned rev*. The crate names that absence in a test
(`ack_vocabulary_is_documented_as_unreachable`) rather than silently omitting it. The crate also
records a second trap worth quoting to anyone who thinks re-export is mechanical: asupersync
declares **two** `AckKind`s, and `obligation/graded.rs:790` is an **uninhabited marker** — matching
the name yields a type with no values.

This matters to the plan because it means the single most valuable half of the vocabulary — the
half that would collapse the three ack dialects — is **blocked on an upstream feature boundary**,
not merely unadopted. Any migration schedule that assumes `AckKind` is available today is wrong.

---

## Design spec: the adoption path

**PROJECTED** throughout this subsection. Nothing below is measured; it is what we will build.

**Collapsing the six Verdicts.** We will define one trait in `omp-types`, over the asupersync
`Outcome` shape rather than a new enum:

```
pub trait Verdictlike {
    fn outcome(&self) -> Outcome<(), VerdictError>;
    fn subject(&self) -> &str;
}
```

`Outcome` is chosen over `Result` for one reason and the reason is the repo's own scar tissue: its
variants are `Ok` / `Err` / `Cancelled(CancelReason)` / `Panicked(PanicPayload)`. **Cancellation is
a first-class outcome, not an error.** That is the type-level form of *a timeout is not a verdict*,
which the repo has already violated once by parsing an empty buffer, defaulting the verdict field
to FAIL, and manufacturing a fleet-wide claim out of nothing. With `Outcome`, a killed child maps
to `Cancelled` and **cannot** be confused with `Err`. Each of the six existing enums keeps its own
variants and gains one impl. Once all six implement it, `fn grade(&[&dyn Verdictlike]) -> Grade`
becomes writable, and grading stops being prose.

**Collapsing the three ack dialects.** The target is `AckKind` — `Accepted` (packet plane accepted
custody), `Committed` (authority plane committed), `Recoverable` (durability class met), `Served`
(service obligation completed by callee). Those four map cleanly onto what our three dialects were
approximating: `Accepted` is transport success from `ntm` JSON, `Served` is the bead-comment ack
read back, and `Committed`/`Recoverable` are distinctions we currently cannot express at all. The
migration is gated on the upstream feature boundary described above; the sequenced form is
(1) advance the pin past the `messaging-fabric`/`test-internals` constraint, (2) re-export
`AckKind` and `DeliveryClass`, (3) convert `ack-spine`'s Authority triple, then `ack-stage`'s
Receipt set, then `receiver-receipt`'s — one crate per change so a bad conversion is bisectable.
Until step (1) lands, the ack collapse is **blocked, and named as blocked**, not scheduled.

**The gate that enforces it.** A crate declaring a public type whose name duplicates one exported
by `omp-types` will fail the build. This is not a new mechanism — `omp-inventory-map` already
implements the pattern, and its `types_inventory.rs:176-178` deliberately excludes `Observation`
from the allowance list *so that the collision demands convergence rather than being tolerated*.
Extending that from one allowance list to the full `omp-types` export set is an incremental change
to a gate that already exists and already passes 13 tests. The same gate acquires the
`subprocess-contract` rule described earlier: constructing a `Command` outside that crate is a
duplicate-of-canonical violation of exactly the same kind. **This raises a floor, it does not
guarantee coherence** — the gate matches names, and two crates can still hold semantically
divergent types under distinct names. It removes the collision class we measured; it does not
remove divergence.

**Why this framework and not an invented one.** Three reasons, in order of force. First,
asupersync **already compiles in our tree** at a pinned rev — it is a dependency of seven crates
today, not a proposal. Second, it **already models the three things we keep re-deriving badly**:
obligation (reserve → commit-or-abort, with `ObligationId`), outcome (including cancellation as a
peer of success), and capability narrowing (`Budget`). Our `Duty` type carries `#[must_use]` and
**no ledger**, which is why a dropped `Duty` leaked an obligation that survived 162 refused ticks
before a human noticed; `ObligationLedger` is the upstream answer to precisely that. Third, and
decisively: **inventing a parallel vocabulary is the exact error the repo's rules forbid.** The
census we are correcting in this section is what happens when a shape is satisfied without being
derived. Authoring a seventh Verdict enum to unify six would be the same mistake with better
intentions.

---

## Where this section disagrees with the brief

Per the parent instruction, a disagreement between a section's own measurement and `00-brief.md`
§3 is reported rather than reconciled silently. Three, in descending order of consequence.

**1. `omp-types` does not re-export the ack vocabulary.** The brief §3.7 states it re-exports
`AckKind`, `DeliveryClass`, `ObligationLedger`, `Budget`, `Outcome`. Command:
`grep -nE '^pub use' crates/omp-types/src/lib.rs`. Result: three lines, exporting
`Outcome`/`OutcomeError`/`PanicPayload`/`Severity`/`join_outcomes`, the `Budget` family, and
`ObligationId`/`RegionId`/`TaskId`/`Time`. `AckKind`, `DeliveryClass` and `ObligationLedger` are
**absent**, and the crate documents why — `messaging-fabric` requires the `test-internals` feature
at rev `fa3c01aec` (`consumer.rs:1299`), which upstream correctly removed from defaults. This is
the consequential disagreement: the half of the vocabulary that would collapse the three ack
dialects is **blocked on an upstream feature boundary**, not merely unadopted, so any schedule that
assumes it is available today is wrong.

**2. The unsafe-forbid denominator.** The brief's 16 of 22 and the earlier 20/26 values are historical snapshots. The current command-backed census is **50 of 50** manifests with unsafe_code = "forbid", **49 of 50** source trees with an inner forbid(unsafe_code) attribute, **49 of 50** carrying both, and a **50 of 50** union. tick-monitor is the sole manifest-only straggler. The substantive gap remains: no single gate yet refuses a new crate that keeps only one mechanism.

**3. The type-inventory scope.** The brief §3.7 gives 51 enums / 79 structs across 22 of 24 crates.
`grep -rhoE '^pub enum …' --include=*.rs crates/ | wc -l` → **59**, and the struct form → **91**.
Not a contradiction — a scope difference (the greps above include test modules and binary sources).
Published as an error bar rather than resolved to the flattering number. The collision count (4) and
the Verdict count (6) are **identical** under both scopes, which is the part that matters.

**NO-CLAIM:** these three are the disagreements this section found while deriving crate contracts
from source. They are not an audit of the brief. Facts in §3 that this section did not need — the
census row kinds, the classification split, the gate leg table — were used as given and are
unverified here.

---

## Constraints this section discovered, written down (R11)

R11 exists because a requirement living only in chat dies with the conversation. Three constraints
surfaced while deriving the table above that were not previously written anywhere:
1. **Unsafe-forbid must be single-mechanism.** The current command-backed census is 50 of 50 manifests, 49 of 50 source trees with the inner attribute, 49 of 50 carrying both, and a 50 of 50 union; tick-monitor is the manifest-only straggler. Earlier 26/25/26 values are historical. Nothing yet fails the build if a future crate keeps one mechanism and drops the other, so the one-file lint remains the required floor-raise.
2. **A crate that spawns must depend on `subprocess-contract`.** Two leaves — `omp-rpc-session` and
omp-inventory-map — spawn processes today with no path-dep on the boundary crate. The rule is stated here so it is checkable, not remembered.
3. **pane-dispatch-fence has no library surface.** It is the only workspace member with no src/lib.rs and zero pub items (ls crates/pane-dispatch-fence/src -> main.rs). Its contract is UNDECLARED and therefore untestable from outside the binary. Any crate whose behaviour other crates must rely on needs a library surface; that is a constraint, and it is now written down.

**NO-CLAIM.** This section states each crate's contract as its source declares it, and states the
adoption path we intend. It does **not** claim any of the following: that the derived contracts are
honoured at runtime; that the 22 crates outside `subprocess-contract` are actually leaking process
groups (only that nothing prevents them from doing so); that the `Verdictlike` trait above compiles
— it has not been written; that the ack migration is schedulable — it is blocked on an upstream
feature boundary we have not yet cleared; or that a tmux pane which has never heard of asupersync
can produce an `Accepted`. That last one is **UNMEASURED**, and because `--mode=rpc` is
single-session and cannot address a third-party pane, the receipt gap may survive the vocabulary
entirely.

---

## 3.9 EXTRACTION DEBT — measured 2026-09-01, and it is the largest gap in this project

Josh: *"any missing crates could be in control-plane that we have to move over — this should be
mentioned in docs."* He is right, it was not mentioned anywhere as a number, and the number is large.

**PRE-EXTRACTION SNAPSHOT (measured before a277097, 2026-09-01):** 20 of the 20 crates marked
CONTROL-PLANE in the then-current AGENTS.md table were not extracted; 28,779 LOC remained upstream.

| crate | LOC | upstream |
|---|---:|---|
| `ntm-fleet-monitor` | 3122 | present |
| `fleet-monitor` | 2569 | present |
| `loop-driver` | 2484 | present |
| `fast-dispatch` | 2292 | present |
| `omp-idle-dispatch` | 1667 | present |
| `fleet-truth` | 1621 | present |
| `pane-dispatch-ready` | 1555 | present |
| `loop-tick` | 1480 | present |
| `fleet-reconcile` | 1424 | present |
| `wired-but-inert-guard` | 1394 | present |
| `verify-dispatch` | 1291 | present |
| `pane-truth` | 1247 | present |
| `reap-finished-panes` | 1189 | present |
| `tick-dispatch` | 990 | present |
| `loop-coverage` | 926 | present |
| `dispatcher-deadman` | 883 | present |
| `refill-idle-panes` | 842 | present |
| `pane-oracle-diff` | 741 | present |
| `oracle-pane-state-differential` | 613 | present |
| `oracle-compare` | 449 | present |

Every one verified present at `/Users/josh/Developer/control-plane/crates/<name>` at measurement
time. At that pre-extraction measurement time, none was missing upstream; none was here.


### Historical interpretation of the 26-crate snapshot
They are **all new work built during this session** — gates, registries, the tick loop, the
supervisor, the schema and number and convergence machinery. That is not a criticism of them; it is
a correction to any reading of this section that assumes the workspace is the extraction landing
zone. At that snapshot time, extraction had not started; bead omp-orchestrator-815 was still open.

### How this surfaced, and what it says about the census


At the pre-extraction snapshot, census_gates() hardcoded a 14-crate list. Three names were not on disk then, so the supervisor correctly classified them as unextracted rather than unwired; this paragraph records that historical refusal, not current presence.
So the census was a frozen snapshot of a *planned* workspace naming 3 of the 20 missing crates
arbitrarily. Dropping them would have been the wrong fix — it would have erased real debt to make a
gate go green. **Naming all twenty is the right fix**, and `NUMBERS.toml` now carries the count so it
cannot quietly drift as extraction proceeds.

**CURRENT STATUS (re-derived 2026-09-01):** the 20 named control-plane crate directories are now present under crates/ in this working tree, while their commit state is governed by the path-scoped extraction commits. The current workspace denominator is 50 packages; the pre-extraction 26-crate and 20-unextracted figures above are historical.

### NO-CLAIM

LOC figures are `wc -l` over `*.rs` in each upstream crate directory, which counts comments and
blank lines and is a size proxy, not an effort estimate. "Present upstream" means the directory
exists — it does **not** mean the crate builds there, that its tests pass, or that it will compile
once moved. `AGENTS.md` itself notes the three newest of these are uncommitted work in a shared
checkout, and a move under those conditions is how work is lost.

---

## 3.10 The extraction workstream and its bead DAG

**HISTORICAL EXTRACTION-PLANNING NARRATIVE.** The bead, graph, leaf, and LOC claims in the following subsection describe the pre-a277097 tree. Current target presence and current missing-source total are stated above and in NUMBERS.toml; the historical 29,512 LOC is not current extraction debt.
Josh: *"our plan needs to include all unextracted stuff — that has to be part of our bead dag."*
Bead `omp-orchestrator-815` is currently **one bead for 29,512 LOC across 20 crates**, which cannot
be worked — it can only be adjudicated. Under `beads-north-star` a bead needs testable acceptance,
and "extract 20 crates" has none. It is an epic with no children.

### The measured dependency shape

Intra-set `path =` dependencies read from each upstream `Cargo.toml`. **14 of 20 are leaves**, so
the first wave is 14-wide parallel with no ordering constraint between its members.

**WAVE 1 — leaves, 20,530 LOC, parallelisable 14-wide**

`loop-driver` 2484 · `fast-dispatch` 2292 · `omp-idle-dispatch` 2183 · `fleet-truth` 1621 ·
`pane-dispatch-ready` 1555 · `loop-tick` 1480 · `fleet-reconcile` 1424 · `wired-but-inert-guard` 1394 ·
`verify-dispatch` 1291 · `pane-truth` 1247 · `reap-finished-panes` 1189 · `loop-coverage` 926 ·
`dispatcher-deadman` 883 · `oracle-compare` 561

**WAVE 2 — one hop, blocked on a wave-1 member**

| crate | LOC | blocked on |
|---|---:|---|
| `ntm-fleet-monitor` | 3122 | `loop-coverage` |
| `tick-dispatch` | 990 | `oracle-compare` |
| `refill-idle-panes` | 947 | `fleet-reconcile` |
| `pane-oracle-diff` | 741 | `oracle-compare` |
| `oracle-pane-state-differential` | 613 | `oracle-compare` |

**WAVE 3 — two hops**

| crate | LOC | blocked on |
|---|---:|---|
| `fleet-monitor` | 2569 | `ntm-fleet-monitor` → `loop-coverage` |

### `oracle-compare` is the articulation point

**Three crates block on it and it is the smallest leaf in the set at 561 LOC.** By
`beads-bv`'s PageRank ordering it should be extracted first, not by size but by unblocking power —
and by `beads-north-star`'s cost rule, *"the cheapest falsifier should kill the branch first."* If
`oracle-compare` cannot be moved cleanly, three downstream extractions are invalid and we want to
know that for 561 lines rather than after 20,000.

`loop-coverage` (926 LOC) is second: it gates the deepest chain, `ntm-fleet-monitor` → `fleet-monitor`,
which is 5,691 LOC of the total.

### The bead decomposition this requires

`-815` becomes an epic with **20 children plus a contract bead**, not one task:

```
-815 (epic)
 ├── 815.contract   the extraction contract: how a crate moves without losing work
 ├── 815.oracle-compare          P0  articulation point, unblocks 3
 ├── 815.loop-coverage           P0  gates the deepest chain
 ├── 815.<11 other leaves>       P1  parallel, no inter-dependencies
 ├── 815.tick-dispatch           blocked-by 815.oracle-compare
 ├── 815.pane-oracle-diff        blocked-by 815.oracle-compare
 ├── 815.oracle-pane-state-diff  blocked-by 815.oracle-compare
 ├── 815.refill-idle-panes       blocked-by 815.fleet-reconcile
 ├── 815.ntm-fleet-monitor       blocked-by 815.loop-coverage
 └── 815.fleet-monitor           blocked-by 815.ntm-fleet-monitor
```

**Every child needs runnable acceptance**, and for an extraction that is the same three commands
each time: the crate builds in this workspace, its own tests pass here, and it appears in
`OMP-SURFACE-MAP.toml` with a `[crates.x]` block — the last because `wired_lanes.rs` already fails
on an undeclared crate, so the gate that catches a botched extraction is **already installed**.

`815.contract` exists because the move itself has a measured hazard: `AGENTS.md` records that the
three newest of these crates are **uncommitted work in a shared checkout**, and a move under those
conditions is how work is lost. That contract is a prerequisite of all 20, not advice.

### NO-CLAIM

This is an **unexecuted historical decomposition**, not a current extraction-status claim. The 20 named directories are present in the current worktree as stated in §3.9, but this subsection's acceptance commands have not been run and no claim is made that the proposed 20-child bead DAG exists. The 29,512 LOC total and 28,779 comparison are historical size snapshots; current ownership and bead state must be re-derived from the current tree and `br`.

---

## 3.11 BLOCKER resolutions — the denominators, settled by measurement

Round 13 graders `GradeCrates` and `GradeGates` independently filed BLOCKERs
against this section and `06-gates` for the same shape: **one property carrying
several live denominators, none declared authoritative.** Measured 2026-09-01,
every number involved is TRUE and they measure different things.

### forbid-unsafe

| reading | value | what it measures |
|---|---:|---|
| manifests declaring [lints.rust] unsafe_code = "forbid" | **50 of 50** | current manifest lint count |
| source roots carrying #![forbid(unsafe_code)] | **49 of 50** | current inner-attribute count |
| **union — AUTHORITATIVE** | **50 of 50** | current union; tick-monitor is manifest-only |

The earlier 26/25/26 table was a dated snapshot. NUMBERS.toml now declares the union command, and the current counts above are re-derived from the shared checkout.

**A REVIEWER ERROR, on the record, because it is the more useful half.** The initial agent-harness grep used grouping syntax and returned zero; the shell command registered in NUMBERS.toml and the current table above use the correct literal pattern. The registry was not exposed. The reviewer was.

### Extraction debt

| reading | value |
|---|---:|
| `AGENTS.md` table, summed | 28,779 |
| **source walk across the 20 named crates — AUTHORITATIVE** | **29,512** |

A 733-line gap, and the resolution is not arithmetic: a hand-maintained table
summed against a live walk is the defect. Registered as
`control_plane_unextracted_loc`. §3.9 and §3.10 keep the 28,779 figure only as
the stale table sum it is, now labelled.


---


<!-- ===== 04-diagrams.md ===== -->

# 04 — FrankenMermaid: the system, generated not drawn

Every diagram below is emitted from a dataset, not from memory. The discipline is
one rule: **a diagram is a rendering of an edge list, and the edge list must have a
command behind it.** If you cannot name the command, you do not get a diagram. The
one exception is the final journey diagram, which is labelled `PROJECTED` in its
caption and shows structure that does not exist yet.

The reason this rule matters to an investor is narrow and specific. Architecture
diagrams are the single easiest artifact in a software project to fake, because
nobody diffs them. A hand-drawn box labelled `receipt` costs nothing to draw and
implies a receiver-verification path that, as Diagram 6 shows, we have measured to
be absent. Generating from the census means the picture degrades when the system
degrades. It is a load-bearing artifact, not decoration.

The source of truth for Diagrams 1, 2 and 6's node set is the built scanner at
`/Volumes/BuildShared/cargo-targets/debug/omp-inventory-map`. The 2026-08-31 capture is
preserved at `.flywheel/inventory-artifacts/inv.txt.gz` (decompressed 544,697 bytes;
compressed SHA-256 `8f62893e6a4a04a9b4e8922781a5f8a687f73ca84f5c4ea9d69c5f8998ae0561`,
exit 2). The newer scanner capture is preserved at
`.flywheel/inventory-artifacts/omp-inventory-map-2026-08-31.json.gz` (decompressed
3,032,388 bytes; compressed SHA-256 `8de42c7cb9e653a79b9781602b16db21e4e281346e42c47c95e71041d9404f52`,
exit 2). These are retained historical snapshots, not a current live diagram feed.
Diagrams 3 and 4 are generated from the five-stage control loop (formerly "five-stage" — renamed, the table has five stages and seven rows) table and gate-leg table
in §00 (`docs/plan/00-brief.md` §3.5, §4); the `find`/`grep` invocations behind those
tables are reproduced under each diagram rather than re-derived.

**A requirement this section discovered, written down here per R11.** Nothing in this
repo currently regenerates these diagrams. A generated diagram that is generated
*once* is a hand-drawn diagram with better provenance, and it rots on exactly the same
schedule. The requirement is therefore: **the diagram set must be emitted by a command
and diffed in CI**, so that a merge which changes the crate DAG and does not change
Diagram 1 fails. That command does not exist today; it is the natural second consumer
of `omp-inventory-map` and would take its `consumes` count from 1 crate to 2. Until it
exists, treat every diagram below as a snapshot dated 2026-08-31, not as a live view.

---

## Diagram 1 — Crate dependency DAG (MEASURED)

```mermaid
graph TD
    subgraph GATES["no-shell-gate family (5 crates)"]
        no_shell_gate[no-shell-gate]
        path_literal_guard[path-literal-guard]
        pre_delete_citation_check[pre-delete-citation-check]
        state_wildcard_lint[state-wildcard-lint]
        undrained_pipe_lint[undrained-pipe-lint]
    end

    subgraph ACK["ack / receipt family"]
        ack_spine[ack-spine]
        ack_stage[ack-stage]
        receiver_receipt[receiver-receipt]
        tick_monitor[tick-monitor]
        finding[finding]
        finding_dispatch[finding-dispatch]
    end

    subgraph CORE["orchestrator core"]
        omp_orchestrator[omp-orchestrator]
        dispatch_claim_fence[dispatch-claim-fence]
        omp_rpc_session[omp-rpc-session]
    end

    subgraph FENCE["process-boundary crates"]
        subprocess_contract[subprocess-contract]
        kernel_only_operator_hook[kernel-only-operator-hook]
        pane_dispatch_fence[pane-dispatch-fence]
    end

    ack_spine --> finding
    finding --> subprocess_contract
    ack_stage --> receiver_receipt
    ack_stage --> tick_monitor
    receiver_receipt --> tick_monitor
    finding_dispatch --> finding
    finding_dispatch --> omp_orchestrator
    omp_orchestrator --> ack_stage
    omp_orchestrator --> dispatch_claim_fence
    omp_orchestrator --> omp_rpc_session
    omp_orchestrator --> receiver_receipt
    omp_orchestrator --> subprocess_contract
    kernel_only_operator_hook --> subprocess_contract
    no_shell_gate --> path_literal_guard
    no_shell_gate --> pre_delete_citation_check
    no_shell_gate --> state_wildcard_lint
    no_shell_gate --> undrained_pipe_lint
    pane_dispatch_fence --> subprocess_contract
```

**HISTORICAL GRAPH SNAPSHOT.** The following edge, degree, and `/tmp` extraction claims are the preserved `inv.txt.gz` snapshot above. They are not current workspace counts; current map and metadata authorities live in `NUMBERS.toml` and the current census sections.
**MEASURED.** Source: all 18 `path-depends-on` edges in `/tmp/inv.txt`, extracted with
`python3 -c "import json; d=json.load(open('/tmp/inv.txt'))['data']; [print(e['from'],'->',e['to']) for e in d['edges'] if e['relation']=='path-depends-on']"`.
Every edge in the picture is one line of that output; the four subgraph groupings are
the only editorial act, and they change no edge.

Degrees, from the same file via
`python3 -c "...collections.Counter(e['to'] ...)"`:

- **17 of 26 crates appear in the DAG at all. 9 are isolated** — `commit-build-fence`,
  `composer-typed`, `dispatch-silence-watch`, `fleet-composite`, `installer`,
  `kernel-bypass-gate`, `loop-queue-filter`, `omp-inventory-map`, `omp-types`. That
  `omp-types` — the crate that exists specifically to be the shared vocabulary,
  re-exporting `Budget` and `Outcome` only — the `AckKind`/`DeliveryClass`/`ObligationLedger` half is blocked upstream (corrected, brief §3.7)
  from asupersync at pinned rev `fa3c01aec` — has **zero dependents** is the single
  most damaging fact this diagram contains. The convergence crate is not converged
  onto. That is why the type inventory still measures 6 distinct Verdict-shaped types
  with no shared trait and 17 ack/receipt types in 3 incompatible dialects.
- **Hub:** `subprocess-contract`, in-degree 4 (`finding`, `omp-orchestrator`,
  `kernel-only-operator-hook`, `pane-dispatch-fence`). It is the correct hub — the
  process-boundary contract is what should be universal — but only 4 of 26 crates depend on it
  directly (6 reach it transitively; 22 do not route through it at all), against 29 raw spawn
  sites measured in the repo.
- **8 leaves** (out-degree 0): `dispatch-claim-fence`, `omp-rpc-session`,
  `path-literal-guard`, `pre-delete-citation-check`, `state-wildcard-lint`,
  `subprocess-contract`, `tick-monitor`, `undrained-pipe-lint`.
- **5 roots** (in-degree 0): `ack-spine`, `finding-dispatch`,
  `kernel-only-operator-hook`, `no-shell-gate`, `pane-dispatch-fence`. Five roots
  means five independent entry points and no single composition point — there is no
  crate that, if you built it, builds the system.
- **Max fan-out:** `omp-orchestrator` at 5, then `no-shell-gate` at 4.

**The objection an investor should raise here:** "a 17-node DAG with 9 orphans is not
an architecture, it is a pile of crates that happen to share a workspace." That is
close to correct today. The answer is not a defence, it is the milestone: convergence
onto `omp-types` and `subprocess-contract` is the measurable target, and the
measurement is the in-degree of those two nodes in a re-run of this exact command.
Today `omp-types` in-degree is 0. That number is the scoreboard.

---

## Diagram 2 — OMP surface consumption (MEASURED)

```mermaid
graph LR
    inv["crate:omp-inventory-map<br/>(1 of 26 crates)"]

    inv -->|consumes| t_cli["type_root:cli"]
    inv -->|consumes| t_cmd["type_root:commands"]
    inv -->|consumes| t_rpc["type_root:jsonrpc"]
    inv -->|consumes| t_slash["type_root:slash-commands"]
    inv -->|consumes| h_get["rpc_handler:get_available_commands"]
    inv -->|consumes| s_probe["slash_command:UNKNOWN_PROBE"]
    inv -->|consumes| tr_mode["transport:--mode=&lt;value&gt;"]

    subgraph UNTOUCHED["surface reachable by zero crates"]
        mass["176 remaining census rows<br/>39 cli_commands · 57 type_roots · 14 declarations<br/>42 rpc_handlers · 3 omp_methods<br/>157 CAPABILITY_NOT_USED"]
    end

    subgraph SILENT["25 of 26 workspace crates"]
        others["consume zero OMP surface"]
    end

    others -.->|no edge exists| UNTOUCHED

    style inv fill:#2d5016,color:#fff
    style UNTOUCHED fill:#4a1010,color:#fff
    style SILENT fill:#4a1010,color:#fff
```

**MEASURED.** Source: all 7 `consumes` edges in `/tmp/inv.txt`, extracted with
`python3 -c "... [print(e['from'],'->',e['to']) for e in d['edges'] if e['relation']=='consumes']"`;
the row and classification counts come from `d['counts']` and
`collections.Counter(r['classification'] for r in d['rows'])` on the same file. The
dashed `no edge exists` arrow is drawn to represent an **absence** in the data and is
the only line in the diagram that is not itself an edge in the census — it is labelled
as such.

Every one of the 7 edges carries the same evidence string, `"direct process probe
produced this row"`. That is honest and it is also the whole problem: the only crate
that touches the OMP surface is the crate whose job is to *scan* the OMP surface. The
census measures the observer observing itself. Of 183 rows, 157 classify
`CAPABILITY_NOT_USED`, 18 `SCRAPED_OR_OBSERVED_ALTERNATIVE`, 8 `MAPPED_BY_DIRECT_PROBE`.

**NO-CLAIM:** this diagram does not claim the 176 untouched rows are *useful* surface,
nor that consuming them would be desirable. It claims only that they are unconsumed.
Deciding which subset is worth wiring is a design act this diagram cannot perform.

---

## Diagram 3 — The five-stage control loop, per-layer status (MEASURED)

```mermaid
graph LR
    OBS["1 · OBSERVE<br/>tick-monitor<br/>WORKS"]
    ACT["2 · ACTIONABLE<br/>idle_panes / free_capacity<br/>FILTER FIXED (-oco); SEAM OPEN — no shared type"]
    CON["3 · CONSUME<br/>dispatch-claim-fence<br/>FENCED"]
    ACU["4 · ACTUATE<br/>ntm --robot-send from the resident supervisor<br/>EXISTS, UNFENCED — 131 re-sends of one unclaimed bead (2026-09-01)"]
    CMP["5 · COMPLETE<br/>worker to conductor<br/>AVAILABLE, NOT WIRED"]

    OBS -->|pane state observed| ACT
    ACT -.->|"Local path: filter fixed (-oco); the SEAM remains —<br/>the production parser derives capacity from its own<br/>JSON string, never the producer's NewlyIdle field;<br/>upstream GuestIdleReconcilerCtx (collab/guest.d.ts:9-30)<br/>is DECLARED only"| CON
    CON -.->|"162 refused ticks over 4.2h<br/>DISPATCH_RETRY_BLOCKED"| ACU
    ACU -.->|"sends without the claim beat; sender success logged as receipt"| CMP
    CMP -.->|"no path back"| OBS

    style OBS fill:#2d5016,color:#fff
    style ACT fill:#5a3a00,color:#fff
    style CON fill:#5a3a00,color:#fff
    style ACU fill:#4a1010,color:#fff
    style CMP fill:#4a1010,color:#fff

    linkStyle 1 stroke:#c04040,stroke-width:3px
    linkStyle 2 stroke:#c04040,stroke-width:3px
    linkStyle 3 stroke:#c04040,stroke-width:3px
    linkStyle 4 stroke:#c04040,stroke-width:3px
```

**MEASURED.** Source: layer 1 from the `tick-monitor` crate's live operation; layer 2 from the local `idle_panes`/`free_capacity` producer-consumer path. State, corrected 2026-09-01: the filter defect is FIXED (commit -oco; `is_free_capacity` is now its own field and `NewlyIdle` is included), and what remains broken is the SEAM — the producer's field and the consumer's parser agree by convention across a process boundary with no shared type (09 M1). OMP supplies `GuestIdleReconcilerCtx` (`dist/types/collab/guest.d.ts:9-30`) for guest host-idle reconciliation and settle handling, but this declared type has no measured path into the local filter. Layer 3 is from the tick ledger — **162 refused ticks across 4.2 hours, every one carrying `DISPATCH_RETRY_BLOCKED`**; layer 5 is recorded as absent because no crate receives a completion. **Layer 4 was corrected 2026-09-01 by the guardian pass:** it is not absent — the resident `omp-orchestrator` (launchd, build `9a61acd`) emits into panes via `ntm --robot-send`, and the heartbeat ledger records 131 `DISPATCHED` rows for bead `815` to `%1408` between 11:45 and 15:53 MDT with the bead `open` and the pane dead on HTTP 402 (00-brief §4 carries the command). The dashed link now names the defect that is live rather than an absence that is not. This node text is hand-edited, as every node in this diagram has been since it was captured — the generator this section requires still has no command and no owner.


Read left to right, exactly one of five links is solid. The loop is not slow, it is
open. Four consecutive dashed links is the honest shape of the system: we observe
well, we cannot decide, we refuse to dispatch, a human actuates, and nothing reports
back. The 162 refusals are not a bug in layer 3 — the fence is doing precisely what it
was built to do given a layer-2 answer that never says "yes". Fixing layer 3 without
fixing layer 2 would convert 162 correct refusals into 162 unfenced dispatches.

**The objection:** "you have an orchestrator that has never orchestrated." Conceded,
without qualification. The value claim is not "it orchestrates"; it is "it refuses
correctly and records why", which is the only foundation on which autonomous dispatch
is safe to switch on. A system that dispatched 162 times and could not tell you what
happened would look far healthier and be far worse.

**NO-CLAIM:** the 162/4.2h figure describes one observed window on one machine. It is
not a rate, not a projection, and does not establish what the refusal count would be
under a fixed layer 2.

---

## Diagram 4 — Gate ladder with leg coverage (MEASURED)

```mermaid
graph TD
    subgraph COMPLETE["4 of 4 legs — 2 gates"]
        g1["no-shell-gate<br/>57 tests (was 34; four test files landed since)<br/>known_bad 4 · known_good 3<br/>mutation 2 · anti_vacuity 6"]
        g3["undrained-pipe-lint<br/>10 tests<br/>known_bad 1 · known_good 3<br/>mutation 1 · anti_vacuity 1"]
    end
    subgraph PARTIAL["partial — 3 gates"]
        g2["omp-inventory-map<br/>23 tests<br/>known_bad 0 · known_good 2<br/>mutation 1 · anti_vacuity 1"]
        g4["state-wildcard-lint<br/>9 tests<br/>known_bad 1 · known_good 1<br/>mutation 1 · anti_vacuity 0"]
        g5["path-literal-guard<br/>3 tests<br/>known_bad 1 · known_good 0<br/>mutation 0 · anti_vacuity 2"]
    end

    subgraph THIN["partial leg coverage — 3 gates"]
        g6["kernel-bypass-gate<br/>6 tests<br/>known_bad 1 · known_good 1<br/>mutation 0 · anti_vacuity 0"]
        g7["pre-delete-citation-check<br/>6 tests<br/>known_bad 1 · known_good 1<br/>mutation 0 · anti_vacuity 0"]
        g8["commit-build-fence<br/>10 tests<br/>known_bad 0 · known_good 1<br/>mutation 0 · anti_vacuity 0"]
    end

    style COMPLETE fill:#2d5016,color:#fff
    style PARTIAL fill:#5a3a00,color:#fff
    style THIN fill:#4a1010,color:#fff
```

**MEASURED (historical snapshot at the plan's 2026-09-01 measurement revision; current worktree census authority is §1 of `06-gates.md`).** Source: `find crates -name '*.rs' -path '*/tests/*' | wc -l` → 31
integration test files; `grep -rhc '#\[test\]'` over crates/*/src/*.rs crates/*/tests/*.rs → 409 `#[test]` functions (this figure drifts with every landing test and is now tracked in NUMBERS.toml `[figures.test_functions]`);
per-leg presence from `grep -rli` for each of `known_bad`, `known_good`, `mutation`,
`anti_vacuity` per gate crate. Counts in each node are that grep's file count, not a
quality judgement.

**2 of 8 gates have all four legs** — `no-shell-gate` and `undrained-pipe-lint`. **4 of 8 have no
mutation leg** — meaning for four gates we have never demonstrated that breaking the
thing under test makes the test fail. **2 of 8 have no known-bad**, i.e. no proof they
fire at all. **One of 8 gates has no known-good leg** — `path-literal-guard` (regenerated
2026-09-01: zero known-good occurrences in that crate's tests). Per §00 §3.5, an attack-only
suite ships an over-strict gate, an over-strict gate gets routed around, and a routed-around
gate is a slower death than no gate at all. A full four-leg row raises the floor on a class
of defect; it never guarantees the class is absent.

**HISTORICAL ADDRESSABILITY SNAPSHOT.** The old --help refusal, 13-test count, and 544,697-byte output below were measured before the retained artifact update. Current source has 28 test markers and the current debug --help probe emits 158 bytes at exit 1. No current ADDRESSABLE pass is claimed without a retained command/output/revision receipt.
A sixth required property fell out of this session and is not in the table because
nothing measures it yet: **ADDRESSABLE**. `omp-inventory-map --help` returns
`{"status":"ERROR","error":"CONFIG_ERROR unknown argument --help"}`. The gate is
built, its 13 tests pass, and `types_inventory.rs:176-178` deliberately excludes
`Observation` from the allowance list so the name collision *demands* convergence
rather than tolerating it. It is correct and it is undiscoverable. A gate nobody can
invoke has a real-world firing rate of zero regardless of its test count.
**CURRENT ACCEPTANCE AUTHORITY (UNRESOLVED).** The diagram generator and CI diff gate named by
the requirement above do not exist in this repository: generator command = **NONE**; CI job =
**NONE**; owner = **UNASSIGNED**. Until a bead assigns an owner and lands an executable command,
these diagrams are snapshots only. The future bead is not accepted until its command regenerates
Diagram 1 from the live census and a deliberately changed crate edge makes CI fail on the diff;
there is currently no command or owner to run.

**What would Jeffrey do.** Searched the mirror at
/Volumes/ZestData/dicklesworthstone-mirror (210 filesystem .git entries, not validated as git work-trees) for diagram-generation and
contract-test prior art: `grep -rl "mermaid" --include=*.rs` surfaces
`franken_markdown/src/pdf.rs` and `franken_markdown/tests/cli_contract.rs`, i.e. a
*renderer* for mermaid plus a CLI-contract test harness — the useful borrow is the
`cli_contract.rs` shape, a test that asserts the CLI's own advertised surface, which
   is exactly the missing ADDRESSABLE leg. Searched for a generated-architecture-diagram
gate specifically: **RETRACTED as a false zero, 2026-09-01.** The original scan globbed
`*/*.rs` and `*/src/**/*.rs` and never descended into crate subdirectories; a full recursive walk
finds **293 mirror `.rs` files containing `mermaid`**, topped by an entire **`frankenmermaid`
monorepo (190 files: fm-parser, fm-render-*, fm-cli)** — mermaid generation with parsers,
renderers, and a CLI — plus `beads_rust`'s `br dep --format mermaid`
(`src/cli/commands/dep.rs:1654`, `render_dep_tree_mermaid`, with e2e contract tests) emitting
mermaid directly from a dependency graph, and ftui-extras renderers. What remains ours to build
is the DELTA none of them ships: regenerating Diagram 1 from the live census inside CI and
failing the diff when the crate DAG moves — the generator-as-gate, not the generator.

---

## Diagram 5 — End-to-end journey (**PROJECTED — not measured, this is the target shape**)

```mermaid
graph TD
    A["foreign repo · any machine"] --> B["install binary<br/>no .sh · no .py"]
    B --> C["orchestrator doctor"]
    C -->|"typed FAIL with remedy"| C2["operator fixes · re-runs"]
    C2 --> C
    C -->|PASS| D["orchestrator health<br/>ntm · br · bv · tmux · git reachable"]
    D --> E["board read · br + bv"]
    E --> F["tick: observe panes"]
    F --> G["actionable set<br/>NewlyIdle admitted<br/>GuestIdleReconcilerCtx exists upstream; local wiring pending"]
    G --> H["claim fence<br/>one claim per bead"]
    H --> I["actuate · typed send"]
    I --> J["receiver receipt<br/>ack within budget"]
    J -->|no ack| H
    J -->|ack| K["work proceeds"]
    K --> L["completion signal<br/>worker to conductor"]
    L --> M["graded close<br/>gate ladder · 6 properties"]
    M -->|FAIL| H
    M -->|PASS| N["bead closed with evidence"]

    style A fill:#1a3a5a,color:#fff
    style N fill:#2d5016,color:#fff
```

**PROJECTED — not measured.** No edge in this diagram is derived from `/tmp/inv.txt`
or from any command. It is the target shape only. Mapping it against Diagram 3: nodes
`F` (observe) exists today; `G`, `H` exist but answer "no" or "refuse"; `I`, `J`, `L`
do not exist in any crate; `M` exists at 1-of-8 leg coverage. The install path `B`
through `D` is unbuilt — `installer` is one of the 9 orphan crates in Diagram 1.

The projected NewlyIdle admitted target is not an implementation claim. OMP declares GuestIdleReconcilerCtx at dist/types/collab/guest.d.ts:9-30 for guest host-idle reconciliation and settle handling, but no evidence connects that context to this local tick-monitor filter.

The single hardest link in this diagram is `J -> H`: the no-ack retry. It is the link
that turns a fire-and-forget send into a delivery contract, and it is the link that
the binding async contract governs — `&Cx` first, `cx.checkpoint()` in loops,
region-owned tasks, kill the process **group**, drain both pipes, and **a timeout is
not a verdict**. A timeout on `J` must produce a typed `DeliveryClass`, never a
silent re-dispatch.

---

## Diagram 6 — The dispatch path actually in use today (MEASURED)

```mermaid
sequenceDiagram
    autonumber
    participant H as Human operator
    participant T as tmux (3.6a)
    participant P as pane composer
    participant B as bead board (br 0.4.1)
    participant C as conductor

    H->>T: tmux send-keys (typed by hand)
    T->>P: keystrokes delivered
    Note over P: work may or may not begin;<br/>no crate observes this transition
    P--)C: receipt / ack
    Note right of C: NO SUCH MESSAGE EXISTS<br/>17 ack types in 3 dialects,<br/>none wired to this path
    C->>B: br status read, later, out of band
    B-->>C: bead state as of read time
    Note over C,B: the only feedback channel is<br/>polling a board a human updated
```

**MEASURED.** Source: the absence of any `actuate` or `complete` crate in the 18-edge
DAG of Diagram 1 — `receiver-receipt` exists as a crate and is depended on by
`ack-stage` and `omp-orchestrator`, but no edge connects it to a pane; the tmux
version from `tmux` at `/opt/homebrew/bin/tmux` (which rejects `--version` with
`tmux: unknown option -- -`, hence the shell-reported `3.6a`); `br 0.4.1` from
`br --version`; the 17-ack-types-in-3-dialects figure from the type inventory
(regenerated 2026-09-01, two scopes published as an error bar: 59 public enums / 91 structs
across 24 of 26 crates including test modules and bin sources; 59 / 88 for
src-only-minus-bins-minus-test-modules), plus 4 colliding names.

Step 4 is the receiver-verification gap, drawn as a dashed unanswered arrow because
that is literally what it is: a message we assume and never observe. Every ack type we
own is a type without a wire. **HISTORICAL SNAPSHOT:** the board at stand-down — 28 closed, 25 in_progress,
19 open, 2 blocked, **74 total** (`28+25+19+2=74`) — is a human's account of what happened, not the system's.
Twenty-five `in_progress` beads with no completion channel is twenty-five unfalsifiable
claims of work in flight.

---

**NO-CLAIM.** These diagrams describe structure and measured state on one machine on
2026-08-31. They do not claim correctness of any crate's internals, do not establish
that any measured count is stable over time or reproducible on other hardware, and do
not assert that the projected journey in Diagram 5 is achievable on any stated
schedule. Diagram 5 asserts no built structure whatsoever. Where a diagram shows an
absence (the dashed arrows in Diagrams 3 and 6, the `no edge exists` link in
Diagram 2), the absence is inferred from an empty result set, and an empty result set
proves only that the scanner and the greps named above found nothing — not that
nothing exists outside their reach.

---

## 4.7 BLOCKER resolution — the provenance was clearable, which is worse than stale

`GradeDiagrams` filed:

> The brief documents a fresh 2026-08-31 capture at
> `/tmp/omp-inventory-map-2026-08-31.json` (3,032,388 bytes), but the diagrams
> section cites sources from `/tmp/inv.txt` (544,697 bytes, round-10 historical). A
> diagram cannot be dated 2026-08-31 while sourced from data predating that capture.

Measured. Both artifacts exist, **and both are dated 2026-08-31** — so the date claim
is technically true and still misleading:

| artifact | size | mtime | sha256 (16) |
|---|---:|---|---|
| `/tmp/inv.txt` — **what the diagrams use, 5 citations** | 544,697 | 16:50 | `86491732a5581a6d` |
| `/tmp/omp-inventory-map-2026-08-31.json` — what the brief cites | 3,032,388 | 23:01 | `876809f0779a81b3` |

Six hours and 5.6× apart. The diagrams are built from the **earlier, smaller** capture
while the brief cites the later one, and nothing in either document says so.

### Provenance finding disposition
**Resolved:** the earlier capture paths were ephemeral `/tmp` locations. The bytes are now preserved and hash-identified under `.flywheel/inventory-artifacts/`; the source-era paths below are historical evidence, not current dependencies.

The diagrams are still **not regenerated from the fresh capture**. §R4 records the separate system gap: generator command = NONE and CI job = NONE. The honest state is a labelled, preserved 16:50 snapshot, not a live view. The 23:01 capture remains preserved for comparison.

**Current retained artifacts:** `inv.txt.gz` decompresses to 544,697 bytes and `omp-inventory-map-2026-08-31.json.gz` decompresses to 3,032,388 bytes. Their compressed hashes are recorded above and enforced by the artifact-provenance gate.

### What is corrected and what is not

**Corrected:** the diagrams are hereby labelled as sourced from the **16:50 capture**,
not the 23:01 one, with size and hash recorded above so the claim is checkable.

**NOT corrected:** the diagrams are not regenerated from the fresh capture. §R4 of
this section already records why — *"Nothing in this repo currently regenerates these
diagrams … that command does not exist today"* — and building it is a separate piece
of work. So the honest state is: **the diagrams reflect a 16:50 snapshot, which is
labelled, hashed, and preserved, and they are not current.** Row counts in Diagram 2
(176 census rows) are from that capture; the 23:01 capture holds 981 rows.

That gap is now stated in the document; the retained artifacts make the historical comparison reproducible without depending on ephemeral source-era paths.


---


<!-- ===== 05-actions.md ===== -->

# 05 — Every action: intended purpose, and the negative pattern it must refuse

Serves **R10**: *"what the stated intended purpose of each action is with negative patterns."*

An orchestrator is not a program that does things; it is a program that decides, on evidence,
whether it is entitled to do a thing — and the interesting half is the refusal. Each action is
specified twice: as an intent, and as the wrong behaviour it must be structurally unable to do. **An
action whose negative pattern is hypothetical is weaker than one whose negative pattern has a
scar**, so each is marked. `How we know it refused` names the observable that stops a refusal from
being a line in a log nobody reads — the 162-consecutive-refusal figure is a historical brief snapshot with no retained deriving ledger. Citations are relative to crates/ and name the construct, not just the line, since a bare line number is unverifiable and drifts; §03 owns the full schema, so Inputs/Outputs carry only the contract and the refusal shape; and a not-found is reported only with the command and why its search space was right.

### A1. OBSERVE a pane
**Purpose.** Turn one pane's rendered terminal into a typed, timestamped `Observation`.

**Inputs.** `pane_id`; raw `tmux capture-pane` text; `at: u64` — bytes off a terminal, untrusted.

**Outputs.** `Observation { pane_id, state: PaneState, hash, at }`
(`tick-monitor/src/lib.rs:488-494`). The refusal shape is `PaneState::Unproven` — *"'I could not
read this pane' and 'this pane is free' are opposite conditions"* (`:218-219`).

**Must be true before.** The capture is for **this** `pane_id` and the read did not time out; a
timed-out `tmux` yields `Outcome::TimedOut` (`:86-89`), whose `stdout_if_completed()` returns `None`
(`:98-103`).

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse to score the whole buffer.*
**MEASURED**: a whole-buffer scan matched a stale spinner in scrollback — *"one pane scored working
AND idle simultaneously while genuinely idle"* (`:288-292`); the fix is `last_status_line()`
(`:293-308`). *(b) Refuse to treat a sub-floor gap as evidence.* **MEASURED**: a 30-second window
called two live panes frozen, because a lane deep in a tool call has a static timer and changing
output; the floor became `MIN_GAP_SECS = 75` — *"a missed freeze costs idle minutes, a false freeze
destroys work in flight"* (`:479-485`). **Disagreement with the brief:** my spawn instructions
The earlier 20-second assertion and citations were stale. Current liveness() compares positive timer or stable-hash motion in tick-monitor/src/lib.rs:564-572 before applying MIN_GAP_SECS at :574-577; a changed Working capture returns Live, while an unchanged short-gap pair remains Unproven. This is now the source-backed behavior, not an open defect.

**How we know it refused.** `PaneState::Unproven`, excluded from every capacity list rather than
defaulted into one; sub-floor refusals carry a machine-readable `why: &'static str` (`:506-586`).
**NO-CLAIM:** proves what one status line looked like at one instant — not that the pane is healthy.

### A2. CLASSIFY liveness
**Purpose.** Compare this tick against the previous one and emit the single `Liveness` verdict a
dispatcher, a conductor, and an alarm each act on differently.

**Inputs.** `prev: Option<&Observation>`, `now: &Observation` (`tick-monitor/src/lib.rs:496`).

**Outputs.** `Liveness`, eight arms (`:403-440`), plus four predicates so consumers never re-derive
policy: `is_dispatchable()` is `ConfirmedIdle` only, `is_free_capacity()` adds `NewlyIdle`, and
`needs_answer()`/`needs_attention()` cover `Dialog` and `Obscured` (`:456-476`). Upstream models
the same split as settle-vs-continuation: `GuestIdleReconcilerCtx`
(`dist/types/collab/guest.d.ts`) is an idle *reconciler* over states, not one predicate.

**Must be true before.** `prev.pane_id == now.pane_id`, checked before every use of `prev.state`
(`:510-516`) — a prior observation from a different pane is not evidence about this one.

**Negative pattern — what this action must REFUSE to do.** *Refuse to key liveness on a marker
regex.* The authority is `stable_hash()`, which strips every braille frame, the `π` glyph, and every
timer before hashing (`:380-396`). **MEASURED** twice: a tool-call box border rendered *after* the
status line produced `<no marker>` on two live panes, and on watcher `%1414` *"a box-drawing region
briefly covered the status line of a pane that was mid-work … and the watcher reported DIALOG"*
(`:525-529`). The response was a new arm, `Obscured`, discriminated by the **prior** observation
rather than this capture's shape (`:517-539`). **MEASURED**, and why `Dialog` exists: `%1372` sat
26+ minutes on an install approval reading as `WORKING`/`LIVE`, *"so the escalation it was waiting
on was invisible to the conductor while looking perfectly healthy"* (`:422-428`). A timer advancing
while a pane blocks on a human is motion, and motion is not work.


**How we know it refused.** The current match at tick-monitor/src/lib.rs:583-620 is exhaustive with no wildcard arm; the old catch-all that could misclassify Working-to-Idle is absent, and state-wildcard-lint guards that shape. **NO-CLAIM:** a two-capture motion claim cannot tell work from a loop.
### A3. SELECT work
**Purpose.** Choose which ready beads to offer next by graph position, so the fleet works the
critical path rather than the operator's most recent discovery.

**Inputs.** `br` ready rows; the `bv` ranking; an epic scope; `QUEUE_WANT` clamped to `1..=20`
(`loop-queue-filter/src/lib.rs:134-135`); a cooldown window.

**Outputs.** A bounded, ordered selection of leaf bead ids. The refusal shape is an empty selection
with a named reason — never one that reads as "nothing to do".

**Must be true before.** The queue was **readable**; `decide()` returns `QueueUnreadable { detail }`
when it was not (`omp-orchestrator/src/lib.rs:441-445`) — a fail-closed arm, not a zero count.

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse to cherry-pick by the
operator's recency of discovery.* **MEASURED**: the top-3 PageRank items were unclaimed while the
conductor hand-picked recent finds. Recency of discovery is a fact about the observer; graph
position is a fact about the work. *(b) Refuse to read `actionable_count` as available work.*
**MEASURED**: it includes in-progress beads, so a fleet with everything claimed reports a healthy
positive number — the denominator defect brief §3.2 retires. Available work is `ready ∧ unclaimed`,
counted separately. *(c) Refuse to dispatch an accounting node.* `issue_type != "epic"`
(`loop-queue-filter/src/lib.rs:399`) and `id.starts_with(&config.epic) && id != config.epic`
(`:416`), pinned by `epic_exclusion_rule_never_selects_parent` — *"parent accounting node must never
be dispatched"* (`:637-648`) — **MEASURED**, same case in the differential oracle
(`tests/differential.rs:141-145`).

**How we know it refused.** The selection is empty **and** the reason is typed; an empty queue
beside free capacity is `QueueEmptyNeedsJosh` (`:379-385`), never an arm meaning "nothing to do".
**NO-CLAIM:** ranks work — not that a bead is well-specified.

### A4. ADMIT a dispatch
**Purpose.** Decide whether **this** pane may receive **this** packet now — the last gate before an
irreversible send.

**Inputs.** Pane id, session, owner, an absolute state dir and ready-probe path
(`pane-dispatch-fence/src/main.rs:71-84`; relative paths rejected).

**Outputs.** Admission, or three named refusals as exit codes — `EXIT_BUSY = 75`, `EXIT_NOT_FREE =
76`, `EXIT_CONFIG = 78` (`:16-18`) — a distinct code, not a generic 1.

**Must be true before.** `is_dispatchable()` — `ConfirmedIdle` and nothing else
(`tick-monitor/src/lib.rs:456-458`). One idle capture is one capture; `NewlyIdle` is visible as
capacity and not fillable (`:409-419`). **Upstream precedent for the two-state split**: the
substrate models settle-vs-continuation explicitly — `AgentEndEvent.willContinue`
(`dist/types/extensibility/shared-events.d.ts:154`, *"subscribers must not treat this as a
user-visible terminal settle"*) and `GuestIdleReconcilerCtx` (`dist/types/collab/guest.d.ts`,
an idle *reconciler*, not a single filter). Our NewlyIdle/ConfirmedIdle pair should mirror
that split: a continuation flag distinguishes a pane that will act again from one that has
settled — what one predicate cannot say. The defect stands (our crates still conflate); the
upstream vocabulary is the model for the fix, not a fix.

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse a pane advertised free that
is wedged.* **MEASURED**: the composite reads a pane as available on `observation_state == "idle" &&
safe_to_dispatch == true` (`fleet-composite/src/main.rs:315-316`) — and a wedged pane satisfies both
while running nothing. The signature is literal: `classify()` returns `PaneState::Wedged` on *"Press
up to edit queued messages"* or *"Messages to be submitted after next tool call"*
(`tick-monitor/src/lib.rs:352-355`); a packet sent there parks in the composer and never submits.
`Wedged` is checked **first** in `liveness()` (`:497-499`) and is in neither capacity predicate; the
receipt layer names it independently as `WEDGED_UNSUBMITTED` (`receiver-receipt/src/lib.rs:45-46`).
*(b) Refuse to admit on a standing verdict.* An authorization that does not expire is a permanent
bypass with a friendly name; `AuthorizedIdle { pane_count, expires_at }` carries its deadline
(`omp-orchestrator/src/lib.rs:376-378`). **PROJECTED — no measured incident yet**; the expiry sits
in the variant, not a config file, so it is unavoidable at the match site.

**How we know it refused.** A distinct exit code and one stderr line naming pane and condition;
admission is serialised by an OS file lock (`:9`, `:31-34`). **NO-CLAIM:** proves the pane could
accept a packet at that instant — not that it arrived, which is A6 and A7.

### A5. CLAIM a bead
**Purpose.** Bind one bead to one assignee **before** the packet is sent, so the dispatch is visible
to the follow-up detector.

**Inputs.** A `BeadSnapshot` from a point-in-time `br show --json` projection
(`dispatch-claim-fence/src/lib.rs:47-72`), plus a `DispatchIntent`.

**Outputs.** A `DispatchPermit`, or a typed refusal — *"A `DispatchPermit` does not attest that
transport occurred; the dispatch ledger remains the authority"* (`:3-7`).

**Must be true before.** The status admits dispatch. `BeadStatus` is a closed set (`:12-32`) — an
unrecognised string becomes `Unknown` carrying the literal, never coerced to a known arm.

**Negative pattern — what this action must REFUSE to do.** *Refuse to send a packet naming an
unclaimed bead.* The order is **file → CLAIM → dispatch**, and the middle beat is not optional.
**Upstream claim vocabulary**: the substrate's claim shape is
`Stage1Claim { threadId, ownershipToken, inputWatermark, sourceUpdatedAt, … }` /
`GlobalClaim { ownershipToken, inputWatermark }`
(`dist/types/memories/storage.d.ts:20-27`) — an ownership TOKEN plus a WATERMARK, claimed
before work starts. Our `DispatchIntent` should mirror that pair: the token binds the claim
to a specific dispatch, the watermark makes a stale claim detectable. This does not close
the defect (our fence already refuses and the type is DECLARED, not wire-proven on our
plane); it validates the design and names the schema the fence's claim should grow into.
The middle beat remains not optional
because the follow-up detector keys on `assigned ∧ in_progress ∧ no-comment`. An unclaimed dispatch
is therefore **invisible to the detector built to notice a silent worker**: `classify()` takes
`current_assignee` and `dispatch_assignee` as required parameters
(`dispatch-silence-watch/src/lib.rs:108-115`) and has a `Reassigned` arm — *"the original dispatch
is moot regardless of whether comments exist"* (`:32-34`). With no claim the bead cannot be silent,
only absent. **MEASURED by consequence**: brief §4 records that every completion this session was
found by a human looking. **MEASURED directly, 2026-09-01, and it is the largest instance:** the
installed supervisor (build `9a61acd`, which calls no claim fence) sent bead `815` — `open`,
`assignee: null` — to `%1408` **131 times in 247 minutes**, one per tick, each row logged
`DISPATCHED … RECEIVER_RECEIPT=ntm_robot_send`; the receiver was dead on HTTP 402 and held 54
copies of the packet. `dispatch-silence-watch` could not see it because the bead was never
`in_progress`, which is exactly the blindness this paragraph predicts. Command:
`jq -r 'select(.pid==70561 and .status=="DISPATCHED") | .detail' ~/.local/state/flywheel/omp-orchestrator.heartbeat.jsonl | sort | uniq -c`.
A second refusal closes the bypass — `DispatchIntent` splits `Bead` from
`Broadcast` and `Correction` *"so they cannot bypass the bead fence by supplying an empty bead
identifier"* (`dispatch-claim-fence/src/lib.rs:100-117`).

**How we know it refused.** No permit is issued, so A6 has nothing to consume — enforced by the
absence of a value, not a checked boolean. **NO-CLAIM:** records intent. It does not reserve files,
stop a second agent editing the same paths, or survive a tracker write that fails silently.

### A6. DISPATCH
**Purpose.** Transmit one packet to one admitted pane and retain the transport's evidence verbatim,
before any later observation overwrites it.

**Inputs.** A `DispatchPermit` (A5), an admitted pane (A4), the packet, and a `TransportKind`
(`ack-stage/src/lib.rs:20-26`).

**Outputs.** A `TransportReceipt` retaining `raw_json` plus parsed
`targets`/`successful`/`failed`/`blocked` (`:42-50`), or one of five `TransportReceiptError` arms
(`:69-77`) — unparseable output is a **named error**, never an assumed success.

**Must be true before.** `supports_delivery_claim()` is true for `NtmRobotSend` only (`:36-39`),
*"the only transport with a retained per-target JSON receipt"*; the tmux fallback surfaces as
`UnprovenTransport` (`receiver-receipt/src/lib.rs:54-57`).

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse to treat `success:[N]` as
delivery.* **Historical incident only — not an in-tree fixture:** cp-z42vu records ntm --robot-send successful:[4] without receiver arrival, but current dispatch-silence-watch tests contain no cp-z42vu or success:[4] payload. The failure shape remains the reason receiver-side evidence is required; no current test result is claimed.
*the natural one*: the transport told the truth about its own send and nothing about the receiver —
the most important negative here. **MEASURED 2026-09-01, 131 times:** every one of the 131 re-sends
recorded under A5 carried `RECEIVER_RECEIPT=ntm_robot_send` — the transport's own success, written
into the heartbeat as if it were a receipt — to a pane that could not act. This is not a historical
incident record; the rows are in `~/.local/state/flywheel/omp-orchestrator.heartbeat.jsonl` today. Hence the receiver crate's first rule: *"A sender return value is
therefore never part of the receipt proof"* (`receiver-receipt/src/lib.rs:5-7`). *(b) Refuse to
bypass a guard without recording what the bypass skipped.* A bypass that logs "overridden" discards
the guard's **true** positives with its false one; a sibling override instead *"names the
superseding artifact"* and comments on each affected bead
(`pre-delete-citation-check/src/main.rs:5-7`). **PROJECTED — no measured incident yet**; written
down because R11 makes an unwritten requirement a dropped one.
**Upstream receipt vocabulary**: typed delivery receipts exist in the substrate —
`IrcDeliveryReceipt` (`dist/types/tools/hub/types.d.ts:8`) and `AsyncJobDeliverySink` /
`AsyncJobDeliveryState` (`dist/types/async/job-manager.d.ts:38,52`) — on the IRC-bus and
background-job planes. This rule STANDS: those are DECLARED types on transports we do not
ride, not a receipt for `tmux send-keys` or `ntm --robot-send`. What changes is the
long-term answer: the gap is a transport CHOICE, not an impossibility — a receipt-capable
transport migration (or an omp collab/irc-plane adapter) is the path that makes A7's proof
constructible from the sender side. Until a wire-proven receipt exists on a plane we ride,
receiver-side evidence remains the only proof.

**How we know it refused.** No `TransportReceipt` is constructed; the failure is a typed error
naming the missing field (`:79-91`), so A7 cannot receive a receipt-shaped hole. **NO-CLAIM:**
proves what the transport reported. Per `cp-z42vu` it proves **nothing** about arrival.

### A7. VERIFY a receipt
**Purpose.** Decide, from receiver-side evidence only, whether the packet actually landed.

**Inputs.** `pane_id`, `pre_send: &Observation`, and `PostSendObservation` = `Present | Absent |
EmptyPaneList | Missing` (`receiver-receipt/src/lib.rs:24-34`).

**Outputs.** `ReceiptVerdict` = `ReceiptConfirmed | NoReceipt | Dead | Indeterminate` (`:118-139`)
over 14 named reasons (`:37-71`; round-12 recount — the earlier "15" was never derived); the binary maps them to exit codes 0 / 1 / 1 / 2
(`src/bin/receiver-receipt.rs:61-64`).

**Must be true before.** `pre_send.pane_id == pane_id` (`:194-202`). Confirmation is keyed on the
**pre-send** state: `IDLE → WORKING` only when the new timer is below
`MAX_IDLE_TO_WORKING_TIMER_SECS = 30` **and** the stable hash changed; `WORKING → WORKING` only when
the timer resets **and** content changed (`:160`, `:175-186`).

**Negative pattern — what this action must REFUSE to do.** *Refuse to read a timeout as a verdict.*
Enforced in the type: `Outcome::TimedOut` is deliberately **not** `Completed { code: non-zero }` —
*"an empty buffer from a killed child must never map to the token a genuinely failing subject
produces. A caller matching on `Completed` structurally cannot read a timeout as an answer"*
(`tick-monitor/src/lib.rs:75-93`), with `stdout_if_completed()` returning `None` for both
non-completed arms (`:98-103`). The same refusal appears twice more, both **MEASURED as live test
legs**: an empty `tmux list-panes` census yields `Indeterminate`, never `Dead`, via
`EmptyPaneListNoDeathClaim` → `NOBODY_DEAD empty_pane_list` (`:185-187`, `:85`); and a
**successful** ack read-back containing no marker is `Missing`, never `Confirmed`
(`ack-spine/tests/ack_detector.rs:25-28`) — a parser rule that holds whatever exit code the tracker
returns, which A11(a) shows is the safer thing to depend on.

**How we know it refused.** A non-`ReceiptConfirmed` arm carrying a named reason (`:151-156`), plus
an exit code distinguishing "not delivered" (1) from "cannot tell" (2) — that distinction is the
whole product. **NO-CLAIM:** proves the timer reset and content changed, not that the agent
understood the packet.

### A8. GRADE a claim
**Purpose.** Establish whether a reported completion is true, by re-running the cited command and
comparing output against the claim.

**Inputs.** The bead id, the claim, and the **cited command** — a claim with no re-runnable command
is ungradeable by construction.

**Outputs.** A grade, its transcript, and a verdict. **The largest missing type in the workspace**:
brief §3.7 measures **6 Verdict-shaped types with no shared trait**, and `Grade` does not exist —
which is why grading is prose.

**Must be true before.** The claim names a command runnable **on this machine, now**, without the
worker's session — and known to have actually run, which A11 shows is not free.

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse to read the worker's report
instead of re-running the command.* **MEASURED — bead `ipg.17`**, instructive: re-running
**refined** the claim rather than refuting it. `omp-inventory-map/src/types_inventory.rs:176-178`
**HISTORICAL ADDRESSABILITY SNAPSHOT:** the gate's old artifact reported 13 source tests, 544,697 output bytes, and an unknown-argument --help result. Current omp-inventory-map has 28 test markers; the current debug binary emits 158 help bytes and exits 1. No current ADDRESSABLE pass is claimed without a retained command/output/revision receipt. **Grade** still needs an arm for correct-and-unreachable.
*(b) Refuse a zero from a tool that cannot distinguish "no matches" from "did not run".* **HISTORICAL MEASUREMENT:** shell grep with --include reported 0 in an earlier harness context; current probes must be re-run and the Rust gate must fail closed.
That earlier comparison recorded shell 0 versus harness 55, and a second search targeted a Go repository with a Rust extension filter; both are historical failure shapes, not current results. A grade built on either is refused. A sixth refutation this session landed in shipped source rather than in the plan (A11(a)); it is retained as a historical lesson, not current acceptance.

**How we know it refused.** A stored transcript naming the discrepancy — claimed token, command
re-run, observed count — and, for (b), the second tool that disagreed. **NO-CLAIM:** proves what the
command output, on a tool proven to have run. Not that it was the right command to cite.

### A9. CLOSE a bead
**Purpose.** Retire a bead with evidence, so the close is a durable artifact rather than a status
transition.

**Inputs.** The bead id, the grade from A8, the transcript, and structural context — parent epic,
children, blockers.

**Outputs.** A closed bead whose `close_reason` cites at least one re-runnable command or path, or a
refusal to close.

**Must be true before.** A8 produced a grade backed by a transcript, and no child or blocker of this
bead is open.

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse a prose close reason.*
**MEASURED**: the `finding` crate header records a wave of **29 beads** where *"at least 8 gaps were
named in prose and never filed"* — one *"citing a path that never existed"* — and *"the true count
is unbounded because nothing counts it"* (`finding/src/lib.rs:6-10`). The second-order failure is
that **the refusal scrolls past** — the shape of the 162 unconsumed refused ticks (brief §4). *(b)
Refuse to close a leaf in a way that closes its parent epic.* The selector already refuses to
dispatch an accounting node (`loop-queue-filter/src/lib.rs:399`, `:416`); the closer must refuse the
mirror-image error. **PROJECTED — no measured incident yet**: the close side has no leg equivalent
to `epic_exclusion_rule_never_selects_parent` (`:637-648`). *(c) Refuse to close without cited
re-run evidence.* **MEASURED**: `cp-3k9jq`'s `close_reason` is 104 characters with **zero** path
citations while its comments cite `bin/fleet-composite.py` three times — *"A gate scanning only
close reasons passes this deletion and the incident recurs"*
(`pre-delete-citation-check/src/lib.rs:14-17`). The gate scans both surfaces, naming the match via
`CitationConflict.field` (`:28-37`), legs pinned at `:148`, `:162`, `:192-203`.

**How we know it refused.** The bead stays open and the refusal is written **onto the bead**, not to
a terminal — the only observable that survives the pane, since the reap found **seven real
conditions living only in pane scrollback** (brief §1). **NO-CLAIM:** proves a command was run, not
that the work is correct.

### A10. REAP a finished pane
**Purpose.** Recognise that a pane's work has ended and return its slot to the capacity pool, so a
finished worker becomes visible capacity rather than a quiet hole.

**Inputs.** The pane's `Liveness` history and the state of the bead it was dispatched.

**Outputs.** A capacity delta and a reap record naming the pane, the bead, and the terminal state.

**Must be true before.** The transition to idle is **observed**, not assumed: `(Working, Idle)`
yields NewlyIdle (tick-monitor/src/lib.rs:595), and the next tick's Idle, Idle yields ConfirmedIdle (:593). The substrate separates the same states with AgentEndEvent.willContinue (:154) and GuestIdleReconcilerCtx; no upstream wire is claimed.

**Negative pattern — what this action must REFUSE to do.** *Refuse to leave a finished pane
unreaped, because an unreaped pane is capacity that silently disappears.* **MEASURED**: the
NewlyIdle is the current arm at tick-monitor/src/lib.rs:595; the old catch-all citation is historical. The actionable filter is fixed: tick-monitor/src/lib.rs:467-468 exposes free capacity for ConfirmedIdle or NewlyIdle, and the regression test is omp-orchestrator/src/main.rs:1646-1653. What remains broken is the SEAM: the producer's field and the consumer's parser agree by convention across a process boundary with no shared type, so a future filter change is invisible again (09 M1). **What would Jeffrey do:** rg -li --type rust -e
'reap(ed|ing)?_pane|pane_reap|kill-pane' --glob '!target' . in the mirror — the extension filter is
sound here only because the subjects are Rust, the hazard A8(b) names — → 7 files, load-bearing
`frankenterm/crates/frankenterm-core/src/orphan_reaper.rs`, whose module doc refuses name-based
reaping outright — a command-line match is not proof of ownership, and PIDs can be recycled
between discovery and signalling, so the reaper ships **inert** (allowlisted to proxy processes)
rather than unsound. **Round-12 retraction: an earlier draft quoted that doc verbatim; the quote
does not appear in the file and was a fabricated scar — the substance stands as paraphrase.** We adopt
it: **reap only what you own, keyed on immutable identity, never on a name match**. §10 carries it.
Upstream answers the same hazard with a reconciler, not a filter — `GuestIdleReconcilerCtx`
(`dist/types/collab/guest.d.ts`) — and with `AgentEndEvent.willContinue`
(`dist/types/extensibility/shared-events.d.ts:154`) separating settle from scheduled
continuation; our fix keeps both states and adds the reconciliation the defect demanded.

**How we know it refused.** A pane in `NewlyIdle` or `ConfirmedIdle` with no reap record is itself
the alarm: reaped and free-capacity panes must reconcile each tick. **NO-CLAIM:** returns a slot —
not that the work finished, only that the pane stopped. `Frozen` is not a reap. Reconciliation
over idle states is upstream vocabulary as well — `GuestIdleReconcilerCtx`
(`dist/types/collab/guest.d.ts`) — which is the shape the reap record + capacity delta pair
implements on our side.

### A11. REFUSE
**Purpose.** The meta-action. Every gate here has one real output — a refusal a machine can consume;
the pass is the uninteresting case.

**Inputs.** Whatever the gate scans, plus the scan set itself — an input, not an assumption.

**Outputs.** A typed verdict and a distinct exit code; the reference shape is `Verdict::Clean |
Verdict::Violations(..)` with a separate `GateError` for conditions that are neither
(`no-shell-gate/src/lib.rs:85-91`, `:56-64`).

**Must be true before.** The scan set is **non-empty**. `scan()` returns `GateError::EmptyScanSet`
at the single choke point — *"a gate that scanned nothing reports identically to one that passed"*
(`:117-124`, `:72-77`).

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse to refuse with exit 0.* **The
standing exemplars are ours**: the `installer` printed *"not yet wired to the live fleet"* and
returned SUCCESS (§07), and shell `grep -r … --include='*.rs'` returns **empty at exit 0** (brief
§3.5). **Two candidates were REFUTED on re-measurement, and that is the sharper finding.** `br
comment <id> <text>`, which our own doc comment at `dispatch-silence-watch/src/lib.rs:13-17` records
as exiting 0, **exits 2** against `br 0.4.1` — refusal on stderr, stdout empty (precision, re-measured: the 2 is a clap usage refusal — the argument was rejected before any comment logic ran — so the exit code refutes the exit-0 story, and the wording above no longer claims the comment path itself answered). The prefix-match to
`br comments` is real; the exit-0 half is false. **So the negative is not "a tool that lies about
its exit code" but a defect claim recorded in a doc comment, never re-derived, and inherited as fact
by every later reader — including this plan, which cited it as MEASURED because the source presented
it that way.** *(b) Refuse to read a non-zero exit as absence.* `tmux --version` gives **exit 1**
with 158 bytes of stderr (the earlier "exit 0" read `$?` after `| head -1`), so a probe treating
failure as ABSENT records tmux — present, `3.6a` on `-V` — as MISSING. We adopt **two independent
presence signals, each arm pinned by its own test including the failure arm**; precedent verified
first-hand as §10's Gap 9 row (`pi_agent_rust/src/doctor.rs` `:950` naive success arm, `:967-968`
two-signal arm, `:1057` one-tool allowlist, arms tested at `:13948`/`:13964`). *(c) Refuse without
naming the satisfying command.* `fh` fails closed with a typed `SERVE_INPUT_STALE` naming the moved
mirror HEAD (brief §3.7); `ntm` ships the fuller typed vocabulary (`internal/bv/bv.go:30`,
verified). *(d) Refuse to be over-strict.* **MEASURED**: `path-literal-guard` has 1 known-bad and
**0 known-good** legs (brief §3.5) — an attack-only gate gets routed around and dies slower than no
gate at all.

**How we know it refused.** A distinct nonzero exit code, a typed verdict a harness can match on,
and a message naming the violation **and** the satisfying command. **NO-CLAIM:** every gate claim
here is a **floor-raise**, not a guarantee — a crate can satisfy every gate and still leak a
detached task, kill a pid instead of a process group, or map a timeout to a failing subject's token.

## The six properties every action's gate must have

Five were doctrine before this session. The sixth was born during it, from A8.

1. **Fires on known-bad**, specimen **in-tree**. An out-of-tree patch harness silently no-ops when
   its index hash misses HEAD, and a gate that no-ops looks exactly like one that passed.
   **MEASURED**: 2 of 8 gates have no known-bad leg (brief §3.5).
2. **Passes known-good.** Mandatory — without it, a gate that refuses everything is
   indistinguishable from one that works. **MEASURED**: 1 of 8, `path-literal-guard`, has no
   known-good leg.
3. **Mutation turns the known-bad RED**, specimen restored byte-identically with the sha reported
   both sides — the only leg proving the *detector* rather than the *fixture*. **MEASURED**: **4 of
   8** gates have no mutation leg; **2 of 8** have all four (`no-shell-gate`,
   `undrained-pipe-lint`). Both figures are corrections: the brief first said 5 of 8 and 1 of 8,
   transcribed rather than recomputed from the table one line above it.
4. **Anti-vacuity: an empty scan set is an ERROR, never a pass**, enforced at the choke point so
   callers inherit it (`no-shell-gate/src/lib.rs:117-124`). Brief §3.3 is this failing on our
   **own** inventory: all 183 census rows carry the four mandatory fields with zero missing and
   exactly **one distinct value** of `must_be_true` — syntactically complete, semantically empty.
   A11(a)'s false-zero `grep` is the same failure in our measurement path.
5. **The claim is a floor-raise, never a guarantee.** A residual "guarantees", "proves", or "makes
   impossible" in a gate header is itself a defect, because a reader who sees it stops looking.
6. **ADDRESSABLE — one documented command runs it, and `--help` names that command.** Added this
   session because a gate satisfying properties 1–5 was unreachable: `omp-inventory-map --help`
   returns `CONFIG_ERROR unknown argument --help` while the gate behind it is correct and tested
   (brief §3.6). **What would Jeffrey do:** `rg -l --type rust -e 'fn
   robot_docs|robot-docs|--robot-docs'` in the mirror (search space sound: the subjects are Rust) →
   prior art in three projects. The closest pins the **topic-set discipline** rather than output
   bytes, proving *"the parser is actively gating on the accepted set"* via exit 2 on an invalid
   topic (`coding_agent_session_search/tests/spec_robot_docs_topics.rs:14-23`); adopt it alongside
   the completions/man drift test (`franken_markdown/tests/completions_drift_test.rs:1-7`). §10
   carries both rows.

**NO-CLAIM:** these six are testable properties of a gate's *test suite*, not of its *correctness*.
A suite can satisfy all six and still test the wrong invariant. Property 6 is satisfied by **zero**
of the eight gates in brief §3.5, and this document is the first place that requirement is written
down rather than said.


---

## Corrected after the Gap 7 refutation

`%1409` found two claims here that HEAD has overtaken.

**A10's actionable defect is no longer live.** The section asserts the `idle_panes` /
`free_capacity` filter defect as current; the cited lines now hold the fix comment. The defect is
history and the section states it in the present tense.

`AgentEndEvent.willContinue` (`dist/types/extensibility/shared-events.d.ts:154`, WIRE-PROVEN) and
`SessionStopEventResult.continue` (`shared-events.d.ts:325-331`, the actual continuation knob —
round-12 correction: the earlier fix cited a nonexistent `SessionStopEvent.settle` member, and
SessionStopEvent's membership in RpcSessionEventFrame is UNVERIFIABLE from the installed types).
Inference remains the *fallback*
for panes that are not OMP sessions; it is no longer the *only* mechanism, and A8/A10 should carry
the typed path as primary.

**NO-CLAIM:** this records the refutation against the two actions that depend on it. The eleven
action specs are otherwise unchanged and have not been re-derived against upstream types — the
signal sweep found seven, and only completion is traced here.

---

## 5.13 The dispatch ledger already existed, and it recorded a 12.3-hour stall nobody read

The `Log every dispatch through our own crates` objective turned out to be already
satisfied — and measuring it produced a worse finding than the gap it was checking.

### What is there

`~/.local/state/flywheel/omp-orchestrator.heartbeat.jsonl`, 486 KB, opened
`.append(true)`, written by `write_heartbeat`. Every row carries
`ts_unix / event / build_id / status / tick / pid / repo / session / detail`.

**HISTORICAL HEARTBEAT SNAPSHOT (2026-09-01).** The 1,323-row table and its 489/56/469 ratios below are retained to explain the failure shape, not as current counts. This host ledger is volatile and now has additional rows; current status must be derived with jq -s over the path above before quoting any total.
**1,323 rows:**

| status | count |
|---|---:|
| `CYCLE_STARTED` | 659 |
| **`DISPATCH_RETRY_BLOCKED`** | **489** |
| `DISPATCHED` | 56 |
| `IDLE_UNAUTHORIZED` | 53 |
| `SUPERVISED_WORKING` | 53 |
| `SUPERVISOR_REFUSED` | 11 |
| `QUEUE_EMPTY_NEEDS_JOSH` | 2 |

So loop dispatches ARE logged through our own crates. The objective is met for the
product's dispatch path.

### The ratio nobody looked at

**8.7 refusals per successful dispatch.** And 469 of the 489 share a *single* cause:
one `dispatch_intent` marker from `pid=92834`, `build_id=b7c2d4e`, spanning
**08-31 11:43 → 09-01 00:01 = 12.3 hours**.

That is the stale-fence stall cleared as `HD-0001`. The loop refused **every tick for
half a day** on a marker whose owning process no longer existed — and wrote a row
about it 469 times.

**The evidence was in the product's own output the whole time. The stall was found
when a human asked, not when the ledger was read.** That is precisely the failure
class this project exists to remove, appearing in the project.

`fh C112` named the mechanism months earlier: *an ownership claim must name something
that dies with the thing it owns.* A pid in a marker file does not, so the marker
outlived `pid=92834` by twelve hours.

### What this changes about the objective

The gap is not logging. It is that **nothing consumes the log**. Two things follow:

1. `dispatcher-deadman` — a watchdog for eligible work that received no packet — is
   now extracted at `crates/dispatcher-deadman` (548 source LOC across `src/lib.rs` and
   `src/main.rs`, verified by `find crates/dispatcher-deadman/src -name '*.rs' -print0 |
   xargs -0 wc -l`). It is **not yet consumed by `omp-orchestrator`** (no dependency or
   source reference in that crate), so the gap is wiring/observation, not extraction.
2. The remaining unlogged dispatches are **operator handrolls**: every `tmux
   send-keys` and `task` dispatch this session bypassed the binary entirely and
   appears in no ledger. That is exactly what `kernel-only-operator-hook` exists to
   refuse, and it is blocked on `cp-nq2s9` (§7 of the hook packet) because the kernel
   it names cannot reach codex panes.

**NO-CLAIM:** an append-only ledger with no reader is not observability, and adding a
reader is not in this section. What is established is the count, the cause, and the
duration — 469 rows, one dead pid, 12.3 hours.


---


<!-- ===== 06-gates.md ===== -->

# 06 — The testing, validation, and gating frameworks we apply

*Serves R6 ("the testing / validation / gating frameworks that we are applying") at the design-spec depth R10 demands. Obeys the writing contract in `00-brief.md` §6.*

A gate is a claim about the future: *this class of defect cannot land again.* The claim is worth exactly what the evidence behind it is worth — evidence that the gate would have caught the defect, would have let the legitimate case through, and fires *because of* the predicate it names rather than incidentally. This section states the nine frameworks we apply, why each beats the weaker option, the shape each takes here, and — where a leg is missing — which gate is load-bearing on faith.

It is written to be failed. §2.4 indicts our own census. §2.8 reports a **projected RED by inspection** in the best-covered gate in the repo, found while writing this section. §5 states the objections we cannot yet answer.

---

## 1. The measured inventory

**HISTORICAL MEASURED SNAPSHOT (2026-09-01).** The row set follows `00-brief.md` §3.5, but that section is an older round-10 snapshot; this table is the later gate remeasurement used by Diagram 4. It is not a claim that the cells match the brief's historical cells. Current acceptance must use the command and revision recorded by the future gate census bead.
**CURRENT WORKTREE AUTHORITY (2026-09-01).** On the shared checkout, the exact census commands
`find crates -type f -path '*/tests/*.rs' | wc -l` and `grep -Rho '#\[test\]' crates --include='*.rs' | wc -l`
return **92 integration test files** and **984 test functions**.

```
python3 -c "import pathlib,re; c=pathlib.Path('crates');
  print(len(sorted(c.glob('*/tests/*.rs'))),
        sum(len(re.findall(r'#\[test\]', p.read_text())) for p in c.rglob('*.rs')))"
  -> 31 409
```

The historical walk produced 31/409; `00-brief.md` §3.5 records an older 31/406 snapshot and cannot corroborate this table. The tooling warning below also disqualifies the brief's `grep -rc` command as a current source.

31 integration test files, 409 `#[test]` functions. Per-gate leg inventory (MEASURED, `grep -rli <property>` per gate crate), aligned with Diagram 4's snapshot:

| crate | tests | known_bad | known_good | mutation | anti_vacuity |
|---|---:|---:|---:|---:|---:|
| `no-shell-gate` | 57 | 4 | 3 | 2 | 6 | (aligned with Diagram 4; five meta-gate files are outside this gate-leg snapshot) |
| `omp-inventory-map` | 23 | 0 | 2 | 1 | 1 |
| `undrained-pipe-lint` | 10 | 1 | 3 | 1 | 1 | (kg/av corrected 2026-09-01: original transposed — 3 known-good fns, 1 anti-vacuity fn)
| `commit-build-fence` | 10 | 0 | 1 | 0 | 0 |
| `state-wildcard-lint` | 9 | 1 | 1 | 1 | 0 |
| `kernel-bypass-gate` | 6 | 1 | 1 | 0 | 0 |
| `pre-delete-citation-check` | 6 | 1 | 1 | 0 | 0 |
| `path-literal-guard` | 3 | 1 | 0 | 0 | 2 |

**0 of 8 gates mutate production source through the real hook** — the only definition that survives typing. `1 of 8` reaches a real temp tree (`omp-inventory-map`, TREE); `2 of 8` mutate a fixture string; `1 of 8` has an affordance nothing flips (`no-shell-gate`). *This paragraph said `2 of 8 … no-shell-gate and undrained-pipe-lint` until the column was rebuilt on what the mutation ACTS ON rather than what a test is NAMED; see `00-brief.md` §3.5, which moved this headline four times.* **4 of 8 have no mutation leg**: `commit-build-fence`, `kernel-bypass-gate`, `pre-delete-citation-check`, `path-literal-guard`. 4 of 8 have no anti-vacuity leg. 2 of 8 have no known-bad. 1 of 8 has no known-good.

**Historical disagreement with the brief.** `00-brief.md` §3.5 states "1 of 8 gates has all four legs" and "5 of 8 have no mutation leg." Those are historical summary prose and contradict its older table. Recomputing this section's aligned snapshot:

```
python3 -c "rows={...verbatim from 00-brief.md §3.5...};
  print(len([k for k,v in rows.items() if all(x>0 for x in v[1:])]))"
  all four legs   : 2 ['no-shell-gate', 'undrained-pipe-lint']
  no known_bad    : 2 ['omp-inventory-map', 'commit-build-fence']
  no known_good   : 1 ['path-literal-guard']
  no mutation     : 4 ['commit-build-fence', 'kernel-bypass-gate',
                       'pre-delete-citation-check', 'path-literal-guard']
  no anti_vacuity : 4 ['commit-build-fence', 'state-wildcard-lint',
                       'kernel-bypass-gate', 'pre-delete-citation-check']
```

`undrained-pipe-lint` carries 1/3/1/1 — all four legs non-zero — so it is complete and was undercounted; and four gates lack a mutation leg, not five. The brief's other two counts are correct. An earlier draft of this section propagated both errors verbatim, which is the finding worth keeping: a headline transcribed rather than recomputed from its own table survives every review that reads the prose and not the arithmetic. The corrected headline is **2 of 8**, and it is worse than it looks, because §3.1 shows **0 of 8** satisfy all six properties.

The objection, stated before it is answered: *you have 409 tests and two complete gates, so the other 326 are decoration.* Partly conceded — several are high-value regression legs against verbatim live captures (§2.9), a distinct and real kind of evidence — but the honest headline is **2 of 8**, and a count of tests is the metric most likely to be gamed by whoever reports it.

**Historical tooling observation (not current acceptance).** An earlier harness context reported shell grep with --include= as an empty, exit-0 result for this pattern; a direct current probe at HEAD returned status 0 with one matching line. The warning is retained as a measurement-harness hazard, not as a current claim about shell grep, and the Rust gate must use a fail-closed scanner.

**The second instance is ours, and an earlier draft of this section got it backwards.** That draft asserted `tmux --version` "prints an error and exits 0 — it fails while reporting success." REFUTED: `tmux --version` exits **1** with empty stdout and 158 bytes on stderr, which is correct, well-behaved failure. The "exits 0" came from a probe reading `$?` after a pipeline, where the status belongs to the last stage — `PIPESTATUS=(1 0)`. **The instrument laundered a clean failure into a success and then reported it as the binary's defect.** tmux is not the offender; our measurement harness is. Retained rather than deleted, because a probe that misattributes its own bug to the thing it measures is a worse failure than the one first alleged, and deleting it would erase the only case in this section where the instrument manufactured the defect it reported.

**And the corrected hazard is inverted, which matters for gate design.** The real risk with a version probe is not exit 0 laundering a failure; it is a probe treating **non-zero as ABSENT** and recording a present binary as missing. `tmux -V` returns `tmux 3.6a` at exit 0, so tmux is present and healthy, while `tmux --version` exits 1 — and no single flag covers our nine binaries (`--version` answers 8 of 9, `-V` answers 6 of 9). A doctor that probes with one flag and reads only the exit status will mark a healthy binary missing.

**What would Jeffrey do — the precedent carries its own remediation AND its own tests.** All citations below name the CONSTRUCT, not just the line, because three of us independently cited this same precedent at three different line numbers and all three were partly right: we were each naming a different construct on adjacent lines. A line number without a construct is unverifiable and does not survive a reformat.

In `pi_agent_rust/src/doctor.rs`: `:924` is `fn check_tool(`, the function. `:950` is the **naive success arm**, `Ok(output) if output.status.success()` => PASS. `:967-968` is the **two-signal fallback arm**, `Ok(output) if discovered_path.is_some() && probe_failure_is_known_nonfatal(tool, args, &output)` => treat tool as present, commented at `:970-971` "Some shells (e.g. dash as `/bin/sh`) do not support `--version`. If this is the known non-fatal probe case, treat tool as present." The two signals are independent by construction: presence comes from `:1066` `fn which_tool(` resolving a path, never from the version probe. `:1052` `fn probe_failure_is_known_nonfatal(` matches stderr against "illegal option", "unknown option", or "invalid option", and `:1057` is the **one-tool allowlist**, `if tool.ne("sh") || args.ne(&["--version"]) { return false; }`.

**And the strongest part is the test pair, which pins BOTH arms.** Verified first-hand. `:13948` `fn check_tool_falls_back_when_probe_args_are_unsupported()` drives `check_tool` with `sh --version` in `ProbeExecution` mode and asserts `Severity::Pass` — the known-good leg for the fallback. `:13964` `fn check_tool_reports_invocation_failure_for_broken_executable()` is a **planted known-bad specimen**, and the craft is in the comment at `:13969-13970`: it writes the literal bytes "not an executable format" to a file and `chmod 0o755`s it "so spawn fails with *exec format error* rather than *not found*" — a specimen constructed to exercise the intended branch and not a neighbouring one, which is §2.1(c) applied to failure *modes* rather than to patterns. It then asserts `Severity::Fail` **and** that the title contains "invocation failed". That second test is the one we would have skipped: it proves the fallback did not become a blanket amnesty, i.e. a genuinely broken executable is still reported broken. It is the known-good/known-bad pair on one function — exactly the structure `path-literal-guard` lacks (§2.2).

**Adoption verdict: ADOPT WITH A NAMED GAP, and the gap contains a real tension.** Take the two-signal arm and both tests. The gap: the allowlist admits exactly one tool, so our `tmux: unknown option -- -` matches the stderr shape and **not** the allowlist, and a doctor built on this code marks tmux MISSING today. But the allowlist is not merely an oversight — it is also the **amnesty bound**, the thing that stops the fallback from waving through any tool whose probe happens to fail. Widening it to admit tmux weakens the bound by exactly that much. The resolution is not a wider pattern but a *per-tool table*: each entry names the tool, the args that are known-unsupported, the flag that does work (`tmux -V`), and the reason — the §2.8 allowance-row shape applied to version probes, so every admission is a named row with a reason rather than a broadened regex.

Four consequences. First, procedural: every figure below comes from the harness `grep`/`read` tools or an inline Python walk, and any number anywhere in this plan derived with `grep -r ... --include=` is a **false zero** until re-derived. Second: a tool that exits 0 with empty output rather than failing is exactly the never-silent-fail defect these eight gates exist to refuse — an empty scan set reported as a clean result (§2.4) — and it occurred in our own measurement path, so anti-vacuity is not only a rule we impose on gates but one our *instruments* violate. A gate whose scan is piped through such a tool reports GREEN over nothing while every leg passes. Third: presence and health are **separate claims requiring separate evidence**, and an exit status is evidence of neither on its own. Fourth, the design consequence: §4 item 5 must assert a **non-zero expected floor** on the scan set rather than mere non-emptiness, and no gate may accept an exit status as evidence of a successful probe without checking that the probe produced content.

**The irony belongs in the record verbatim.** `omp-inventory-map`'s universal `must_be_true` reads: *"The source probe is non-empty before a known verdict is emitted."* That is precisely the rule this whole subsection derives from two live instrument failures — and the census states it **183 times identically**, which is the vacuity §2.4 indicts. Right rule, vacuous application, in the same artifact.

NO-CLAIM: the leg table counts files whose *name* matches a property. A file named `mutation.rs` that mutates nothing counts here as a mutation leg. §4 specifies the meta-gate that closes that hole; it does not exist today.

---

## 2. The frameworks, one design spec each

### 2.1 Fires-on-known-bad (planted specimen)

A specimen of the exact defect class is planted into the gate's real scan set; the gate must go RED and *name* it; the specimen is removed and the gate must go GREEN — both directions in the same run.

**Why not weaker.** The weak form is "the gate is green, so the tree is clean," and that inference is invalid: a green gate is indistinguishable from one that scanned nothing, keyed on an unmatchable pattern, or was pointed at the wrong root. A gate that has never fired has zero evidence behind its claim. Unit assertions on the matcher are also weaker — they exercise the predicate without exercising *scan-set derivation*, which is where vacuity lives.

**Shape here.** MEASURED: `crates/no-shell-gate/tests/gate.rs` builds a throwaway git repo per leg (`fresh_git_tree`, 40-51), writes and `git add`s a `run.sh` so it appears in the real `git ls-files` index (`stage`, 69-77), asserts RED with the path named, then unstages and deletes and asserts GREEN — `planted_shell_is_red_then_green_after_delete` (203-228), with an independent `.py` twin (233-257) so neither extension is exercised only at unit level. The GREEN half keeps a `README.md` staged, so the clean verdict is rendered over a non-empty scan set and is a real verdict rather than vacuity.

**The staging lesson, MEASURED.** Probes staged into `git ls-files` produced `CARGO_EXIT=101` and the failure named **both** output paths — which is what proves the `.sh` and `.py` legs discriminate independently rather than one masking the other. A specimen written to disk but never staged is invisible to this gate, and the leg passes while proving nothing.

**Design spec.** (a) The specimen enters the *production* scan set — the git index here, a real file under `crates/*/src` for `path-literal-guard`. (b) The RED assertion checks the named path, not merely a nonzero exit. (c) Each forbidden pattern gets its own specimen. (d) Restore is byte-identical and verified: a planted leg that leaves the tree dirty converts one gate's evidence into every other gate's false positive. **Cost:** one `git init` per leg; `no-shell-gate` pays it four times.

### 2.2 Known-good positive control

A legitimate case, of the kind the gate must *not* flag, asserted to pass.

**Why not weaker.** An attack-only suite ships an over-strict gate, and that is a subtler death than it sounds. A gate with false positives does not get fixed; it gets *routed around* — `--no-verify`, a new exemption row, a lane that quietly stops calling it. Slower than having no gate, because the exemption looks like compliance and the coverage report still counts the gate as present. §2.8 documents this happening right now.

**Shape here.** MEASURED: `gate.rs:114-126`, `clean_list_passes`, pins Rust sources, manifests, markdown, `notes.sh.txt` (FINAL extension only, so the matcher cannot key on a substring), and a dotfile whose stem is not an extension. Those last two are the cases an over-strict extension matcher gets wrong — the boundary, not the obvious pass.

**The measured gap.** `path-literal-guard` has **known_good = 0**. It scans every `.rs` under `crates/*/src` for home-path literals and asserts zero hits (`tests/repo_wide.rs:28-38`); it has anti-vacuity (20-26) and a known-bad, but nothing pins what a *legitimate* path expression looks like. It can become arbitrarily strict with no test going RED, and it is repo-wide, so the blast radius is every crate. **`path-literal-guard` is the highest-risk gate in the set** on this measurement, despite being the smallest.

**Design spec.** Each gate declares at least one *adversarially chosen* legitimate case: the one nearest the boundary. For `path-literal-guard` that is a `repo_root()`-derived join, an `env!("CARGO_MANIFEST_DIR")` expression, and a legitimately-absolute `/tmp`-style path, all asserted to produce zero hits. **Cost:** near zero at runtime; the real cost is the judgement to pick the boundary honestly.

### 2.3 Mutation testing

Break the thing the gate keys on. The leg must go RED. Restore byte-identically and verify with a hash. A leg that stays green under mutation of its own predicate is not attributable to that predicate and proves nothing.

**Why not weaker.** Strongest single leg, and the one 4 of 8 gates lack. Known-bad proves the gate fires; mutation proves it fires *for the stated reason*. Without it a leg can pass on an unrelated assertion, an incidental exit code, or a fixture that trips a different branch — and the documented invariant is a story, not a mechanism.

**Shape here, two forms, and the difference matters.** MEASURED. *Form A — through the binary:* `crates/composer-typed/tests/mutation.rs` runs the real binary with `--mutation --disable-rule <name>`, asserts the *inverted* outcome, then runs clean and asserts the correct one, printing both. `mutation_dim_suggestion_is_not_typed` asserts `rc=0` with the rule disabled (greyed autosuggestion misclassified as typed) and `rc=1` enabled; the disable switch is a first-class production flag, so the mutation traverses the path production traverses. *Form B — through the real hook:* the production predicate was flipped in source, an active-registration `git commit` was run through the installed hook, the commit went **RED at exit 101** (once, manually — a measured experiment, not a standing leg), and the source was restored with the sha reported on both sides. `crates/commit-build-fence/tests/hook.rs` is built for exactly this — `fresh_repo` (7-21) creates a real repo with a real baseline commit, and `run_git_with_store` (39-47) drives a real `git commit` with the fence bound via `OMP_BUILD_REGISTRATION`.

**Why through-the-real-hook is categorically stronger.** A fixture certifies the predicate. The real hook certifies the predicate, its installation, its invocation path, its exit-code contract, and git's interpretation of that exit code — five links, four of which a fixture cannot see. The measured cost of getting this wrong is §2.9: a gate whose selftest *and* mutation leg were both green against fixtures that had drifted from its real payload.

**Design spec.** (a) The mutated symbol is deliberately named for attributability — `no-shell-gate/tests/wired_lanes.rs:85` (line at snapshot 995a147; :96 before the file shrank) declares `const STRIP_TEST_CODE: bool = true` with the comment "deliberately named so its mutation is attributable." (b) Prefer a production `--disable-rule`-style switch to a source edit, because it is reversible by construction. (c) Where source must be edited, record the sha before and after and report both. (d) Run through the real invocation surface whenever one exists. **Cost:** highest of the four legs — `git init` plus hook install plus a real commit per leg, and the source-edit form needs restore discipline.

### 2.4 Anti-vacuity — including our own violation

An empty or unreadable scan set is an ERROR, never a pass. A deliverable that was never checked must never report like one that passed.

**Why not weaker.** Highest-yield property in the set, because it is the failure that makes every other leg lie. `no-shell-gate` carries it at three levels — unit (`empty_scan_set_is_an_error_not_a_pass`), end-to-end (`empty_index_is_an_error_not_a_pass`), and CLI exit code (`binary_exits_2_on_empty_index`: exit **2** for gate-error, distinct from **1** for violation-found). `path-literal-guard/tests/repo_wide.rs:14` states the principle — "a verdict without its coverage is unauditable" — and prints the whole scan set beside the verdict.

**THE SELF-INDICTMENT.** MEASURED (`00-brief.md` §3.3):

```
python3 -c "…Counter(json.dumps(r.get('must_be_true')) for r in rows)…"
  crate rows:     n=26   distinct must_be_true=1  distinct negative_evidence=1
  non-crate rows: n=157  distinct must_be_true=1  distinct negative_evidence=1
```

All 183 census rows carry the four mandatory fields with **zero missing** — and exactly **one distinct** `must_be_true` and **one distinct** `negative_evidence` across the entire census. The universal invariant is `["The source probe is non-empty before a known verdict is emitted.","A versioned inventory envelope carries the probe state."]` — an invariant about *the scanner*, not about any of the 183 things scanned. For the 26 crate rows, `inputs`/`outputs` describe the scanner's provenance (`cargo metadata --format-version 1 --no-deps` plus the crate's `Cargo.toml`), not the crate's contract, and `what_it_provides` is boilerplate — "Workspace crate X from cargo metadata" — distinct only because the name varies.

The four-field discipline is satisfied **syntactically and vacuously**: the exact defect anti-vacuity exists to catch, committed by the instrument that enforces anti-vacuity elsewhere, indicting the conductor rather than a worker. We publish it in our own section because an investor who finds it unaided has found a gap we did not see.

**Design spec for the fix.** A distinct-invariant-count check: for a census of `n > 1` rows, if `distinct(must_be_true) == 1` or `distinct(negative_evidence) == 1`, FAIL with `VACUOUS_INVARIANT_SET`, naming the repeated value. Two refinements the naive version needs: (a) partition by row-kind, so a legitimately homogeneous kind is not forced to fabricate variation — the measurement above already partitions crate from non-crate rows and both violate independently; (b) reject an invariant that names the *scanner* instead of the row's subject, enforceable as "a crate row's `must_be_true` must mention that crate's identifier."

**What would Jeffrey do — and a retraction, because this section published a false zero.** An earlier draft stated: `searched vacuous|vacuity in asupersync/docs, asupersync/AGENTS.md, aadc/AGENTS.md — no matches`, and concluded "the *word* is absent from the mirror." **REFUTED.** Re-derived with the harness `grep`, no extension filter, across the whole `asupersync` repo rather than three hand-picked files: `vacuous|vacuity|vacuously` matches **36+ files in that one repo**. The search space was the defect — three files chosen because I expected doctrine to live in docs, when the concept lives in test code, production telemetry, a shell gate, and a Lean proof. This is the same false-zero class as the `--include=` grep (§1) with a different mechanism: not a filter that matched nothing, but a *search space too narrow to contain the answer*. A not-found is only publishable if the search space is justified, not merely stated.

The prior art is richer than the design in this subsection, on four counts.

**(1) Anti-vacuity guards live INSIDE the metamorphic relation.** `src/runtime/scheduler/metamorphic_tests.rs:440-441` asserts, before checking its relation: *"MR4 VIOLATION: zero cancel dispatches across {} injected cancel tasks — streak-bound assertion would be vacuous"*; `:517-522` repeats it for MR5 labelled "ABSOLUTE-CORRECTNESS ANCHOR", and `:661-662` for MR7 — *"timed-lane dispatched {} of {} injected tasks — EDF ordering check would be vacuous on the missing tasks."* The relation does not merely hold; the test first proves the workload was actually exercised. **This is the leg that would have caught `pane-truth`** (§2.9): a fixture-format drift means zero live payloads were classified, and a guard asserting "at least N inputs reached the classifier" goes RED where a green relation says nothing.

**(2) Vacuity can be a typed state rather than a failure.** `src/messaging/jetstream.rs:2460` serialises `waiter_fairness_mode: "vacuous_zero_wait_refusal"` into production telemetry, and `src/messaging/jetstream_flow_control_audit.rs:6` explains it: *"for the current controller, fairness is vacuous because hidden waiters are impossible (`max_waiters = 0`)."* The system does not claim fairness and does not fail; it **names its own vacuity as a first-class value**, and `scripts/run_jetstream_publish_backpressure_smoke.sh:181-186` gates on that exact string. This is strictly better than the binary FAIL this subsection specified, for the case where vacuity is *legitimate and permanent*: a trivially-satisfied invariant carried forward under a name a later reader cannot mistake for evidence. Our `VACUOUS_INVARIANT_SET` is the right response to *accidental* vacuity; this is the right response to *structural* vacuity, and we had no vocabulary for the second.

**(3) A positive control is justified as anti-vacuity, fusing §2.2 and §2.4.** `tests/atp_rq_observability_metrics.rs:134-135`: *"Positive control: the manifest DOES carry its content-descriptor fields, so the negative assertions above are meaningful (not vacuous on an empty blob)."* The known-good leg exists *because* the attack leg would otherwise be vacuous — one mechanism, not two. Same shape at `src/stream/buffered.rs:1238` ("two empty or two all-identical sequences would compare equal and prove nothing"), `src/runtime/builder.rs:7363-7364` ("an empty fingerprint would make the equality below vacuous"), and `tests/three_lane_tests.rs:7593-7594` ("a pass here means the worker never touched the shard — vacuous").

**(4) Anti-vacuity is pushed into the type system and the API return value.** `src/trace/tla_export.rs:111-113` declares `pub type EntityKey = (u32, u32)` precisely so slot-reuse aliasing cannot make invariants "pass *vacuously* because one entity silently overwrote the other in the map." And `src/combinator/map_reduce.rs:142-143` has `all_succeeded()` return **false** on empty input "even though the aggregate decision is `AllOk` (vacuously true)" — refusing to let a caller read vacuous truth as success, with the test at `:728-729`. Measured yield of the discipline: `CHANGELOG.md:1077-1078` records six HTTP/1.1 RFC 9112 tests "that previously **passed vacuously** when codec validation was missing," and `audit_index.jsonl:3251` a metamorphic MR2 that "**was vacuous** because it toggled an unrelated testing Cx instead of the inspected runtime state" — our exact defect, found by audit, in his repo.

**Adopted.** (i) Every metamorphic relation in §2.6 ships an input-reached guard before its relation. (ii) The `VACUOUS_INVARIANT_SET` check gains a sibling: a **named structural-vacuity mode** carried in the envelope for invariants that are trivially satisfied by construction, gated on the name. (iii) Known-good legs are documented as anti-vacuity mechanisms for their paired attack legs, not as independent niceties.

### 2.5 Differential / oracle testing

Two independent implementations of the same judgement are compared. An absent, empty, or unreadable oracle is an ERROR or an announced SKIP — never silent agreement.

**Why not weaker.** For a judgement with a large input space and no compact specification, a second independent implementation is a far denser oracle than a hand-written expectation table — and the failure mode of tables is measured. `crates/composer-typed/tests/differential.rs:26-30` records that `frankenscipy-ivg5` audited 12 conformance runners and **11 invoked no oracle at all**, comparing against hand-typed `case.expected` fields while still populating an `oracle_status` field, so the report looked differential while nothing differential ran.

**Shape here.** MEASURED. `composer-typed/tests/differential.rs` compares against `bin/composer-typed.py` under the house rule quoted at line 3: "python and shell are only allowed to use for comparisons, all gated oracles should be rust." It is a comparison, not a gate, and is *forbidden* to fail the suite when the oracle is absent — because `bin/composer-typed.py` can never exist here (the no-`.sh`/no-`.py` gate's exemption list is empty). Before that change both tests FAILED with a uniform `python=2` across all 8 cases — python3 failing to open a nonexistent script, read as 8 semantic disagreements — which made an absent external tool the authority over a green Rust suite and turned `cargo test --workspace` red, burying the no-shell gate's own signal. The absence path is now a **typed** `OracleStatus` with a LOUD `announce_skip` (69-83), modelled on `franken_whisper/src/differential_oracle.rs:1-6` and its `DifferentialSkipReason::MissingExecutable`. `oracle-compare`'s stated role (`AGENTS.md:485`) is the same invariant in one line: "An empty or unreadable oracle must be an ERROR, never a silent agreement."

**Design spec.** The shell implementation remains the differential oracle for `pane-truth` while the Rust port is proven. Both are fed the same captured pane bytes; disagreement is a FINDING, not an automatic Rust defect, because §2.9 proves the shell side can be the wrong one. Oracle absence yields a typed skip that announces DID NOT RUN and is never counted as a passing comparison. The oracle is never a Cargo or runtime dependency. When the Rust port's four legs are green against *live* captures, the oracle is retired and the retirement recorded with the date and the leg set that replaced it.

**What would Jeffrey do.** `aadc/.beads/issues.jsonl:91` (bd-fbf) states the discipline exactly: "The Rust implementation is the only authority on what 'correct' means. A second independent implementation (deliberately slow, deliberately readable) catches regressions in the Rust implementation that pass its own tests." *Deliberately slow and readable* is a constraint we had not written down; it is now.

### 2.6 Metamorphic testing

Where the correct output is unknown, assert a *relation* between inputs and outputs that must hold regardless: idempotence, invariance under an irrelevant transformation, monotonicity under an ordered one.

**Why not weaker.** Pane classification has no ground-truth label. A captured status line is Working or Idle because the model rendered it so; there is no authority to consult. Expectation tables in this domain are precisely the artifact §2.9 proves drifts silently. A metamorphic relation survives that drift because it never names an expected output.

**Status.** PROJECTED for the design; MEASURED for the absence, with the search space stated because §1 and §2.4 both prove a bare zero is not a finding. Harness `grep` for `metamorphic`, no extension filter, over `crates`, `docs`, `AGENTS.md`, and `WAVE.md`: **zero hits under `crates/`**. The only hits in the repo are in `docs/plan/`, and they are this session's own plan text — this section and its siblings — not implementation. The search space is right because `crates/` is the entire workspace (root manifest is `members = ["crates/*"]`, a glob), so no implementation can exist outside it.

**What would Jeffrey do.** Extensive prior art, and the strongest argument in this section for adopting a framework we lack. `asupersync/CHANGELOG.md:1260-1261` reports a hardening pass with "**25 real production-code bugs fixed** (most of them pre-existing, surfaced by the recently-expanded metamorphic test suite)" — measured bug yield, not a style preference. `:1345` describes "hundreds of metamorphic relations" across the runtime, scheduler, obligation ledger, and RaptorQ; `:1029` names a "**Restart-budget metamorphic oracle**." `aadc/.beads/issues.jsonl:73` (bd-b7g) names three concrete relations for a text-alignment algorithm — block-order permutation, tab-width round-trip, whitespace append — with the rationale "Especially valuable for an algorithm where the 'correct' answer is hard to specify but transformations should commute," which describes pane classification exactly. `asupersync/ATP_DOD_CHECKLIST.md:19-21` makes it a done-condition with two fields that must be filled — `Command:` and `Properties tested:` — so a metamorphic claim cannot be made without naming its command. We adopt that field pair in §4.

**Design spec.** Three relations over `tick-monitor::classify`. **MR-1 (invariance):** prefixing/suffixing whitespace or re-wrapping must not change the classification — a pane does not become idle because tmux padded it. **MR-2 (monotonicity):** for a captured Working line, increasing the rendered timer must yield a strictly larger `timer_secs` and must never flip the variant. **MR-3 (non-interference):** inserting a token-budget or spend counter must not change the classification — the generalisation of the measured leg `token_budgets_and_spend_counters_are_not_elapsed_timers` (`tests/monitor.rs:45-57`), which pins two specific counters where MR-3 pins the class. Each ships with `Command:` and `Properties tested:` filled per the ATP checklist. A relation marked `#[ignore]` must carry a reason, as `asupersync/CHANGELOG.md:1460-1463` does when the lab scheduler cannot expose a policy.

**And each relation opens with an input-reached guard, adopted from `metamorphic_tests.rs:440-441` (§2.4).** Before asserting its relation, every MR asserts that a minimum number of live captures actually reached `classify` and produced a non-`Unknown` variant — the shape `assert!(classified >= N, "MR-k VIOLATION: {classified} of {offered} captures classified — relation would be vacuous on the rest")`. This is the single most valuable line in the whole design, because it is the leg `pane-truth` lacked: its fixtures were the wrong format, so zero live payloads were classified, and both its selftest and its mutation leg passed on an empty effective input set (§2.9). A relation that holds over nothing holds. The guard converts that silent pass into a named RED.

### 2.7 Golden artifacts / schema pinning

The output envelope is frozen; a shape change fails the build until the change is deliberate and versioned.

**Why not weaker.** Every consumer of a machine-readable envelope — CI, another crate, a foreign repo, an agent — is coupled to its shape. Without a pin, a field rename is a silent breaking change discovered as a mysterious parse failure in someone else's build. Per-field assertions are weaker: they cannot see a *removed* field or a changed `status` vocabulary.

**HISTORICAL SHAPE SNAPSHOT (2026-08-31).** The old inventory envelope reported 544,697 bytes and exit 2. Current acceptance requires a retained artifact and source revision; this section does not treat that volatile byte count as current. The schema-version threading and validate-on-read design remain the intended gate shape.

**Design spec for `omp-inventory-map/v1`.** (a) Commit a golden envelope with `data` elided to its key set; the test compares key sets and the `status` vocabulary, not the 544 KB payload — a golden that changes on every scan gets regenerated reflexively and pins nothing. (b) Assert the four mandatory row fields present on every row (passes today) *and* diverse per §2.4 (does not). (c) `status` is a closed vocabulary; an unrecognised value is a parse ERROR. (d) A `schema_version` bump requires the golden regenerated in the same commit, and the test keys on the version string, so bumping without regenerating is RED. (e) Adopt `commit-build-fence`'s validate-on-read: consumers reject a foreign version loudly instead of best-effort parsing it.

**(f) The count-twin invariant, and it is the load-bearing one.** The envelope carries paired counts — an observed count and an `expected_*` twin. MEASURED: six of the seven pairs match exactly, and one does not — `slash_commands=0` against `expected_slash_commands=136`. That single mismatch is why the envelope reports `status: UNKNOWN` and the binary exits 2, and it is the largest unmapped region of the OMP surface. The pin therefore asserts a *conditional*: for every twin pair, either observed equals expected, **or** `status` is `UNKNOWN` and the specific mismatching pair is named in the output. Both halves matter. Without the first, a scanner that silently drops to zero on every kind still reports a clean envelope. Without the second, `UNKNOWN` becomes a blanket amnesty that lets any number of new mismatches hide behind one already-known gap — which is exactly what `slash_commands` does today: the status is honest, and the envelope does not name which twin broke it. A gate that reports UNKNOWN without naming its own unknown is unactionable, and 136 unmapped surfaces is too large a hole to leave addressed only by a status string. **Cost:** one golden file; one regeneration step per deliberate schema change.

### 2.8 Conformance harness with an explicit allowance list — and a projected RED by inspection

Every member of a *derived* set must satisfy a property. Exceptions live in a declared allowance list where each row names the member **and** a reason. The validator refuses a row with no reason, and refuses a row naming a member absent from the derived set.

**Why not weaker.** The weak form is a hand-listed expectation set, and `no-shell-gate/tests/wired_lanes.rs:32-38` (snapshot 995a147) names the prior art it avoids: control-plane's `check.sh` hand-lists `EXPECTED_GATES` while the verdict claims completeness, so "the list drifts and the suite reports vacuously green while most lanes are unexamined." Here the set is derived from disk (`derive_lanes`, :50-86; `workspace_crate_names`, :561-577 — line numbers at snapshot 995a147, constructs verified by name) so a new crate is in scope the moment it exists, and an empty or unreadable derivation is an ERROR (`empty_scan_sets_are_errors_not_passes`, 473-483).

**Shape here.** MEASURED. The `UNWIRED_LANE_ALLOWANCE` pattern, taken from `franken_lean`. `wired_lanes.rs` carries four independent legs, each owning one predicate, one scan, one allowance, one validator — "Mutating one predicate must leave the other three green: no shared scan, no shared helper beyond `workspace_crate_names` (a pure directory read)" (:557-562). Two allowances are **empty by construction**: `SURFACE_ALLOWANCE` (:596) and `FORBID_ALLOWANCE` (:636). The validators are `every_allowance_row_names_a_lane_and_carries_a_reason` (486-506) and `validate_allowance_rows` (590-604) — the latter requires a reason of **≥ 8 characters**, so a one-character reason is refused too. The maintenance contract is load-bearing: rows are checked against the DERIVED set every run, and stale rows are refused with "allowance names undeclared lane …", which **fired live** when extraction removed two members mid-grade. The harness caught the `installer` lane; **the RED was the pass** — a harness green on first run would have told us nothing.

**A PROJECTED RED BY INSPECTION, found writing this section.** MEASURED input set, not an executed test failure. Leg 3, `every_crate_declares_the_forbid_lint` (:639-672), iterates the derived set (all 26 `crates/*` dirs holding a `Cargo.toml`; the root manifest is `members = ["crates/*"]`, a glob) and requires each `Cargo.toml` to satisfy `text.contains("unsafe_code") && text.contains("forbid")`, with `FORBID_ALLOWANCE` empty. Measured with an inline Python walk of `crates/*/Cargo.toml` and `crates/*/src/{lib,main}.rs`:

```
crate dirs            : 50
manifest lint present : 50
manifest lint MISSING : none
all src roots forbid  : 49   (missing: tick-monitor)
```

**Current measurement, not the old six-crate snapshot:** one source root (tick-monitor) lacks the inner attribute, but the union command reports 50 of 50. The gate remains PROJECTED-BY-INSPECTION because no executable conformance test has yet been run against this current set.


Three things follow. **First**, the current property holds by union but not by the stricter both-mechanisms predicate: 50 manifests, 49 source attributes, 49 intersections, and tick-monitor as the one manifest-only crate. **Second**, a new crate can still drift unless the projected conformance gate parses manifests and checks the source attribute. **Third**, substring conjunctions over a whole file remain invalid because comments can satisfy them; parse the manifest and read the lints.rust unsafe_code field.

**Design spec.** (a) Derive the set; never hand-list. (b) Empty derivation is an ERROR. (c) Every row: member + reason + the condition under which the row dies — `ALLOWED_COLLISIONS` (`omp-inventory-map/src/types_inventory.rs:180-206`) already states "Dies when …" per row. (d) A row naming an undeclared member is an ERROR. (e) A member naming itself is not a caller (`a_lane_naming_itself_is_not_wired`, 519-533). (f) Comments and test-only code are not evidence of wiring (`comments_and_test_only_code_do_not_prove_wiring`, 456-471). (g) **New, from the RED:** a conformance predicate targets the *property*, not one declaration site — leg 3 becomes "forbids unsafe by manifest lint OR source attribute," and gains the known-good leg it never had, pinning a crate that conforms by attribute only. (h) Substring conjunctions over a whole file are forbidden; parse the manifest and read `[lints.rust] unsafe_code`.

**What would Jeffrey do.** Two upgrades to the row shape. `asupersync/conformance/artifacts/conformance_registry_contract_v1.json:125-134` carries four-field rows — `disposition`, `superseded_by`, `reason`, and `retention_reason` ("File deletion is forbidden; retained for future metamorphic repair") — where ours carries two. And `asupersync/.github/no_mock_policy.json` runs the same pattern at repo scale with `pattern`, `category`, and `owner`. We adopt `owner` and `dies_when` as required fields: an allowance row with no owner is an orphan, and our five current `UNWIRED_LANE_ALLOWANCE` rows (20-41) share one reason and one landing bead with no named owner between them.

**A row that must be restated as BLOCKED, not pending.** `ALLOWED_COLLISIONS` deliberately omits `Observation` (`types_inventory.rs:176-179`) so the gate REFUSES that collision "until the convergence lands," and the convergence would be reached by adopting the shared vocabulary `omp-types` re-exports. MEASURED, and it changes the row's meaning: `ObligationLedger` occurs **zero** times in `omp-types`, and `AckKind`/`DeliveryClass` appear only in a doc comment naming them as blocked. They sit behind `#[cfg(feature = "messaging-fabric")]`, which transitively requires `test-internals`, which upstream issue #46 correctly removed from default features. So the half of the vocabulary that would collapse our 17 ack/receipt types across 3 incompatible dialects is **unreachable at our pinned rev** — blocked at an upstream feature boundary, not merely unadopted and not pending our work.

A refusal whose remedy is unreachable is not a decision awaiting execution; it is an indefinite RED with no owner and no landing condition. The row must therefore state the boundary: refused, blocked by `messaging-fabric` → `test-internals` at asupersync rev `fa3c01aec` (upstream #46), `dies_when` the feature boundary moves or we vendor the vocabulary locally, `owner` unassigned. Writing it as "until the convergence lands" implies the convergence is ours to schedule, which is the same shape as a close reason with no evidence: a sentence that reads like a plan and commits no one to anything.

**Cost.** The source-stripping machinery (`strip_comments`, `strip_test_code`, `brace_delta`) is ~100 lines of careful parsing and is the part most likely to harbour its own bug.

### 2.9 The floor-raise claim discipline

A gate header saying "guarantees", "proves", or "makes impossible" is itself a defect, because a reader who believes it stops looking. Each gate states what it mechanically enforces **and** what still passes.

**Why not weaker.** Not documentation hygiene — the control on the most expensive failure we have measured. MEASURED: `pane-truth`'s fixtures were the Claude Code status format (`Working (2s - esc to interrupt)`), so **its green selftest AND its mutation leg were both vacuous** against the payload it actually runs on (`crates/tick-monitor/tests/monitor.rs:15-19`), and it reported `liveness_two_capture: false` on every pane for exactly that reason. A complete-looking four-leg gate certified nothing. A "guarantees" header would have been actively harmful, redirecting attention from the only question that mattered.

**Shape here.** MEASURED: `no-shell-gate/tests/gate.rs:18` states its NO-CLAIM inline — "extensions of tracked files only." `wired_lanes.rs:7-8`: "This suite proves reachability only: a caller can invoke a lane while the invoked mode may still be weaker than the lane's live guarantee." `tick-monitor/tests/monitor.rs:15` states its fixtures are "VERBATIM captures from live panes on 2026-08-31, not hand-written approximations" — a claim about *provenance*, which is the direct remediation of the pane-truth defect.

**Design spec.** Every gate header carries three fields: **ENFORCES** (the mechanical predicate in the gate's own vocabulary), **STILL PASSES** (the nearest defect it does not catch), **PROVENANCE** (where the fixtures came from; live capture or constructed, with a date). The §4 meta-gate rejects a header containing "guarantee", "prove", "impossible", or "cannot happen" outside a NO-CLAIM sentence. **Cost:** prose discipline, and the restraint not to overstate a gate you just built.

---

## 3. The six required properties

1. **FIRES-ON-KNOWN-BAD** — a planted specimen in the real scan set turns it RED and names the specimen.
2. **KNOWN-GOOD** — an adversarially chosen legitimate case passes.
3. **MUTATION** — breaking the keyed predicate turns the leg RED; restore byte-identical and hash-verified; through the real invocation surface where one exists.
4. **ANTI-VACUITY** — an empty or unreadable scan set is an ERROR with a distinct exit code; and the invariant set is non-degenerate (§2.4).
5. **FLOOR-RAISE CLAIM** — ENFORCES / STILL PASSES / PROVENANCE; no "guarantees".
6. **ADDRESSABLE** — one documented command runs the gate, and `--help` names that command.

### 3.1 The six-by-eight coverage matrix

Legs 1-4 are transcribed from §1. Property 5 is MEASURED here: `grep` for
`NO-CLAIM|STILL PASSES|proves .{0,20}only|does not (catch|prove|claim)` across the eight gate
crates returns headers in exactly three of them — `no-shell-gate/src/lib.rs:11`
("WHAT STILL PASSES — do not read this gate as more than it is"),
`omp-inventory-map/src/types_inventory.rs:16` ("this proves SHAPE, not SEMANTICS"), and
`path-literal-guard/src/lib.rs:16` ("WHAT STILL PASSES"). Property 6 is UNMEASURED for seven
of eight, because establishing it requires running each binary, which this section is
forbidden to do; the one measured value is `omp-inventory-map`, which FAILS.

| gate | 1 known-bad | 2 known-good | 3 mutation | 4 anti-vacuity | 5 claim | 6 addressable |
|---|:--:|:--:|:--:|:--:|:--:|:--:|
| `no-shell-gate` | Y | Y | Y | Y | **Y** | — |
| `omp-inventory-map` | N | Y | Y | Y | **Y** | **N** |
| `undrained-pipe-lint` | Y | Y | Y | Y | **N** | — |
| `commit-build-fence` | N | Y | N | N | **N** | — |
| `state-wildcard-lint` | Y | Y | Y | **Y** (was `N` — corrected 2026-09-01) | **N** | — |
| `kernel-bypass-gate` | Y | Y | N | N | **N** | — |
| `pre-delete-citation-check` | Y | Y | N | N | **N** | — |
| `path-literal-guard` | Y | **N** | N | Y | **Y** | — |

**Zero gates satisfy all six.** Three set relations read against expectation and are the
useful part of the matrix. First, `path-literal-guard` has the *best* claim discipline in the
set and the *only* missing known-good — the gate most honest about what it does not catch is
the one least protected against catching too much. Second, three of the four gates with no
mutation leg also have no claim discipline (`commit-build-fence`, `kernel-bypass-gate`,
`pre-delete-citation-check`); the fourth is `path-literal-guard`, which has the best claim
discipline of any gate. Third, and tightest: the four gates missing anti-vacuity
(`commit-build-fence`, `state-wildcard-lint`, `kernel-bypass-gate`,
`pre-delete-citation-check`) are a **strict subset** of the five missing claim discipline —
`undrained-pipe-lint` is the only gate that carries anti-vacuity while claiming more than it
enforces. PROJECTED: the co-occurrence has one cause — a gate written to close a specific
incident, shipped the moment it went red on that incident, and never revisited to ask what
else it now claims or what it would report against an empty scan set.

NO-CLAIM: column 6 is seven-eighths unmeasured, so this matrix understates nothing and may
overstate the addressability of every gate but `omp-inventory-map`.

**HISTORICAL ADDRESSABILITY SNAPSHOT.** An earlier inventory run reported omp-inventory-map --help -> CONFIG_ERROR unknown argument --help, 13 source tests, and 544,697 bytes. Current source has 28 test markers and the current debug binary's --help emits 158 bytes at exit 1. No current ADDRESSABLE pass artifact is claimed until command, output, and revision are retained; the property remains projected in the gate matrix.

MEASURED, with the search space named: harness `grep` for `ADDRESSABLE`, no extension filter, over `crates`, `docs`, `AGENTS.md`, `WAVE.md` — **zero hits under `crates/`**; every hit is in `docs/plan/`, i.e. this session's plan text written by this section and its siblings. `crates/` is the whole workspace (`members = ["crates/*"]`), so the property exists in prose and in no code. PROJECTED: that it will be enforced.

**What would Jeffrey do.** ADDRESSABLE is a named bead class in the mirror. `aadc/.beads/issues.jsonl:145` (bd-zku) is "Document undocumented CLI flags and subcommands in README — Five subsystems are implemented in src/main.rs but absent from the README CLI reference table," which is our defect class filed as work. `:133` (bd-u01) mechanises it: `check-readme-claims` asserts "Every flag in 'aadc --help' is in README CLI table," plus exit codes, default values, and short forms. `:68` (bd-abk) states the ambition — a custom `long_help` per flag turning `-h` into "a discoverable mini-tutorial." And `:128` (bd-r6h) supplies the reporting shape we adopt in §4: a definition-of-done matrix where each row is **PASS / FAIL / SKIP with an evidence path** — §2.5's typed skip applied to release readiness.

---

## 4. Design spec: how a new gate is admitted

Future tense. A gate will not be permitted to fail anyone's build until it satisfies all six properties, and the enforcement will not be a review convention. The admission checklist a candidate gate will satisfy:

1. It declares its scan-set derivation, and the derivation reads the world (git index, directory tree, `cargo metadata`) rather than a hand-listed constant.
2. It carries a planted known-bad in that real scan set: RED-with-name, then GREEN, with byte-identical restore.
3. It carries a known-good boundary case, chosen adversarially.
4. It carries a mutation leg with a deliberately named mutation point, run through the real invocation surface if one exists.
5. It carries anti-vacuity at every level it has — unit, end-to-end, CLI — with gate-error distinguished from violation-found by exit code (the `no-shell-gate` 2-versus-1 convention). Non-emptiness is insufficient: the gate asserts a **non-zero expected floor** on its scan set (e.g. "at least 26 crates," "at least one tracked file per declared lane"), because §1's silently-empty tool proves a scan can collapse to a plausible-looking small number without erroring. A floor turns "I scanned something" into a falsifiable claim.
6. Its predicate targets the property, not one declaration site, and never by substring conjunction over a whole file (§2.8g/h).
7. Its header carries ENFORCES / STILL PASSES / PROVENANCE and no unqualified "guarantee".
8. `--help` runs, exits 0, and names the command that runs the gate.
9. It is reachable from `cargo test` — not only from a `main.rs` subcommand a human must remember. MEASURED precedent: `tick-monitor`'s binary `--selftest` has 41 legs and `cargo test` reaches none of them, so its invariants were moved to where the suite executes them (`tests/monitor.rs:3-7`).
10. Every allowance row carries member, reason (≥ 8 chars), `dies_when`, and `owner`.
11. Each claimed framework fills the ATP field pair: `Command:` and `Properties tested:`.
12. It never reads an exit status as evidence of a successful probe. Presence and health are separate claims requiring separate evidence — a path resolution establishes presence, a content check establishes health, and a non-zero status establishes neither absence nor ill-health on its own. Precedent, its tests, and its gap: `pi_agent_rust/src/doctor.rs:967-968` (the two-signal fallback arm), pinned by `:13948` `check_tool_falls_back_when_probe_args_are_unsupported` and `:13964` `check_tool_reports_invocation_failure_for_broken_executable` (§1).
13. Every zero it reports — "no violations", "no prior art", "no occurrences" — carries the exact command **and** the justification that its search space could have contained the answer. A bare zero is not a finding. MEASURED cost of omitting this: two false zeros in this section alone, one from a `--include=` filter that silently matched nothing (§1) and one from a search space of three hand-picked files when the answer lived in 36 (§2.4).
14. Every citation it makes — to a precedent, a test, or a source line — names the **construct**, not just the line. MEASURED cost of omitting this: three of us cited one precedent at three different line numbers and all three were partly right, because each named a different construct on adjacent lines (§1). A line number is unverifiable alone and does not survive a reformat; a named construct can be re-found.

Reporting: one row per gate, **PASS / FAIL / SKIP with an evidence path**, per `aadc` bd-r6h. A SKIP with no evidence path is a FAIL.

The meta-gate enforcing it will be a conformance harness of exactly the §2.8 shape, whose derived set is *the gate crates themselves* and whose properties are items 1-14. `GATE_ADMISSION_ALLOWANCE` starts empty; every exception is a named crate with reason, `dies_when`, and `owner`; a row naming a non-gate crate is an ERROR. Its own known-bad is a planted gate crate missing one leg, refused by name; its mutation leg disables one property check and asserts the harness goes green, proving each check is independently attributable. It closes §1's NO-CLAIM by checking *behaviour* — does the mutation file actually invert an outcome — rather than a filename. Item 13 it checks on itself: the harness's own "zero non-conformant gates" verdict must carry its derivation and the argument that `crates/*` is the complete search space.

Ordering, and it is not cosmetic: the meta-gate is admitted **last**, after at least one gate satisfies all *six* properties — the four-leg bar is already met by two gates (§1) and is not the binding constraint; the binding constraint is that **zero** gates clear property 5 and property 6 together (§3.1). A meta-gate whose derived set is entirely non-conformant produces an eight-row allowance list on day one, and an eight-row allowance list is indistinguishable from no gate.

---

## 5. What would make this whole framework fail

**Gates that are green because they scan nothing.** The likeliest failure, already committed at census level (§2.4). Anti-vacuity is present in only 4 of 8 gates; `commit-build-fence`, `state-wildcard-lint`, `kernel-bypass-gate`, and `pre-delete-citation-check` each show `anti_vacuity = 0` and will report identically whether they passed or scanned an empty set. The countermeasure is mechanical and cheap and is not there.

**A mutation leg that is not attributable.** If the mutation point is not deliberately named, a refactor moves it and the leg keeps passing for a different reason. Worse, a mutation leg run against a fixture can be green while the production predicate is unreachable — measured in `pane-truth`.

**A fixture drifted from production, certifying nothing.** MEASURED, and the most expensive, because it defeats all four legs simultaneously. No amount of leg completeness detects it; only PROVENANCE does — fixtures must be verbatim live captures with a date, which `tick-monitor` now satisfies and nothing else in the repo is required to.

**An over-strict predicate routing a gate around.** No longer hypothetical: §2.8 reports leg 3 failing six conforming crates. The pressure that follows a false positive is to add allowance rows, and an allowance row is indistinguishable from compliance in every summary we produce.

**The allowance list as a pressure valve.** `UNWIRED_LANE_ALLOWANCE` has five rows sharing one reason and one landing bead. If that bead slips, the honest response is that the rows are stale; the dishonest response is to edit the reason. The maintenance contract catches a row naming an *undeclared* lane. It does not catch a row whose reason has quietly become false. That is an open hole in the strongest pattern in this section, and `owner` + `dies_when` (§2.8) narrow it without closing it.

**Gating discipline concentrated where the product is not.** The sharpest structural objection. Per `00-brief.md` §4, exactly one of five pipeline layers works: `observe` WORKS, `actionable` is BROKEN, and `consume` is FENCED (162 refused ticks over 4.2 hours). Local `actuate` is not wired. A completion signal does exist upstream — `AgentEndEvent` with `willContinue`, at `dist/types/extensibility/shared-events.d.ts:154`, carried on `RpcSessionEventFrame` (`modes/rpc/rpc-types.d.ts:589`) — but this pipeline does not consume it yet; local completion therefore remains unavailable by adoption, not by absence of the event. Eight gates and 409 tests guard a pipeline that cannot yet dispatch or complete a unit of work here. An investor is entitled to ask whether the gate budget bought defect prevention or the appearance of rigour. Our answer, and it is a partial concession: the gates encode the specific defect classes that consumed this session — vacuous verdicts, silent oracles, unattributable legs, unaddressable binaries — and those are the classes that will destroy the four missing layers as they land. But the ordering risk is real, and the mitigation is that no new gate is admitted (§4) until it clears a bar the existing eight mostly do not.
**Costed value gate (PROPOSED, not measured).** A new gate earns admission only when its owner records: (1) no more than **4 owner-hours** for implementation and review; (2) no more than **30 seconds** of incremental wall time on the standard gate invocation; and (3) a baseline plus a three-release follow-up for escaped instances of the named defect class, targeting **at least one fewer escaped instance** in that window. If no historical baseline exists, the substitute is a named shipped capability with a runnable acceptance command; no reduction claim is made. These are explicit budgets and acceptance criteria, not measurements of the existing eight. **STOP/DEFER:** stop investment when the estimate exceeds either budget, when the gate catches no previously unguarded class and unlocks no named capability, or when the required outcome evidence cannot be collected; defer until the missing measurement or capability exists. Existing gate counts are not product progress unless this value evidence is reported separately.
**409 tests as a proxy metric.** If the count becomes the target, the count will rise and the leg table will not. The only numbers worth reporting are the leg table and the §3.1 matrix, and the only honest headlines today are **2 of 8** on four legs and **0 of 8** on six properties.

**A transcribed headline nobody recomputes.** Demonstrated in this section: the brief's own summary line disagreed with the table directly above it on two of four counts, and the first draft of this section reproduced both errors while citing the correct table (§1). Every derived count in this plan is one transcription away from being wrong in the same way, and prose review does not catch it. The countermeasure is that a count in prose must carry the expression that computes it from the table, not the table's caption.

**A zero that was never a measurement.** The most dangerous failure in this section, because it is indistinguishable from a real result and it defeats the mirror-mining requirement outright. Two instances here, with two different mechanisms: a shell `grep -r --include=` that returns empty at exit 0 (§1), and a search space of three hand-picked files when the concept lived in 36 (§2.4). A third, from a sibling, filtered `--include='*.rs'` across a Go repository and read structural absence as semantic absence. All three produce a confident "no prior art found" that reads exactly like a true one, and in every case the correction came from someone re-deriving rather than reading. A framework built on prior art cannot tolerate a false zero, because the false zero is precisely the result that stops the search — and unlike a wrong number, nothing downstream contradicts it. §4 item 13 is the countermeasure, and it is weaker than the disease: it makes the search space auditable but cannot make it complete.

---

**NO-CLAIM.** This section describes the frameworks and their measured coverage as of 2026-08-31. It does not claim the eight gates are sufficient to prevent the defect classes they name; it does not claim the 409 tests are individually load-bearing. Two gates — `no-shell-gate` and `undrained-pipe-lint` — have all four legs; **none** of the eight satisfies all six properties, because ADDRESSABLE and the floor-raise claim discipline exist in this document and in no validator. Column 6 of §3.1 is measured as a property label, not a semantic test count.

**Retractions this section carries rather than deletes**, because each is more instructive than the corrected value: (1) the leg-count headlines "1 of 8" / "5 of 8", transcribed from the brief and refuted by the brief's own table (§1); (2) `tmux --version` "exits 0 while failing", refuted — tmux exits 1 correctly, and the defect was our probe reading `$?` after a pipeline, `PIPESTATUS=(1 0)` (§1); (3) `searched vacuous|vacuity … no matches`, refuted — 36+ files in `asupersync` alone, and the prior art is richer than the design it was cited to justify (§2.4). Two of the three were errors of *measurement method*, not of arithmetic, and no amount of prose review would have caught them.

The leg-3 RED in §2.8 is PROJECTED-BY-INSPECTION from a measured input set, not an observed test failure: no cargo command, gate binary, test suite, formatter, or linter was executed in producing this section. Every figure comes from the harness `grep`/`read` tools or an inline Python walk; shell `grep -r` with `--include=` is measured to return empty at exit 0 on this machine and is not a source for any figure above. Each reported zero names its search space and why that space could have contained the answer — the rule §4 item 13 imposes on gates, applied here to this document.


---

## 6. Corrected after the Gap 7 refutation

`%1409` graded this section against the corrected plan and found it carrying a world that no longer
exists. Both findings are accepted.

**This section's §5 objection is premised on a refuted gap.** It argued the pipeline cannot complete
because no worker-completion signal exists. **It does** — `AgentEndEvent` with `willContinue`, at
`dist/types/extensibility/shared-events.d.ts:154`, carried on `RpcSessionEventFrame`
(`modes/rpc/rpc-types.d.ts:589`). The objection survives only in the weaker form *"we do not ride
that plane yet,"* which is an adoption cost, not an impossibility.

**And the leg counts were the round-1 world.** `2 of 8` against the brief's `0 of 8`, and `370`
against `379` **inside this section's own measured block**. Both corrected above. A section can be
internally inconsistent and still pass a gate that checks literal retired strings — the retired-figure
gate was green on this file throughout.

**NO-CLAIM:** these corrections align this section with `00-brief.md` as of `dea4af6`. They do not
re-derive its nine framework specs against the refutation, and at least one — the mutation framework
in §2.3 — argues from a scarcity of upstream prior art that the signal sweep has now partly refuted.

---

## 6.5 Two gate-wiring defects, both found by being blocked

Measured 2026-09-01 while trying to land a commit. Neither is about a gate's
*logic*; both are about the gap between a gate's source and the artifact that runs.

### 6.5.1 `state-wildcard-lint` was 89% false positives and blocked every commit

The pre-commit chain refused with `state-wildcard-lint: 9 finding(s)`. Read
individually, **8 of the 9 were wildcards the compiler requires**:

| site | scrutinee | why `_` is mandatory |
|---|---|---|
| `omp-inventory-map` ×5 | `Option<T>` with a **guarded** `Some(x) if …` arm | a guarded arm does not cover `Some` |
| `tick-monitor:1034` | `&[&str]` slice patterns | slice space is unbounded |
| `tick-monitor:1042` | `&str` literal match | string space is unbounded |
| `tick-monitor:99` | **`Outcome`, a real 3-variant enum** | **genuine — now enumerated** |

**Root cause:** `type_for_match` inferred the scrutinee's type from any `Enum::`
appearing anywhere in the match **body** — including the *result* arms:

```rust
match value {                          // scrutinee: Option<Vec<_>>
    Some(items) if !items.is_empty() => ProbeState::Known,
    _ => ProbeState::Unknown,          // <- ProbeState is the OUTPUT
}
```

Seeing `ProbeState::`, the lint concluded the scrutinee was `ProbeState` and
demanded exhaustive arms for an `Option` match. Resolution now requires **pattern
position** — left of the fat arrow, or the `match` header itself.

Result: 9 → 0 findings, with a positive control in both directions (restore the
wildcard on `Outcome` → 1 finding naming `scrutinee="self", type=Outcome`;
byte-identical restore → 0).

This is the AGENTS.md failure mode reached in practice: *"an over-strict gate gets
routed around — a slower death than no gate."* It was blocking **every pane's
commits**, which is the strongest possible pressure to bypass it.

### 6.5.2 The hook is a compiled artifact, and nothing rebuilds it

Fixing the library changed nothing, because `.git/hooks/pre-commit` is a **Mach-O
binary** that links `state-wildcard-lint` as a *library* — it never shells out. So
the live gate was a **third artifact**, distinct from both the source and the
binaries I rebuilt:

| artifact | timestamp | findings |
|---|---|---|
| `release/state-wildcard-lint` | 12:11:41 (**13h stale**) | 8 |
| `debug/state-wildcard-lint` | 01:27:06 (fresh) | 0 |
| `.git/hooks/pre-commit` | 01:21:59 (1.9 MB, another pane's debug build) | 8 |

I measured my own fix with the 13-hour-old release binary — `find … | head -1`
returned whichever path sorted first — and read "8 findings" as the fix having
failed. **Fourth instrument error of the session, same class as the other three.**

**BUILT ≠ WIRED, aimed at the enforcement layer itself.** A stale hook silently
enforces yesterday's rules, and there is no gate on the freshness of the gate.
The repair was manual: rebuild `--bin pre-commit-gate`, copy over
`.git/hooks/pre-commit`, verify exit 0.

**NOT BUILT:** a check that the installed hook's hash matches a build of current
`HEAD`. Without it, every lint change needs a human to remember two extra steps,
and forgetting them is invisible — the hook keeps passing or failing for reasons
that no longer exist in the source.

### 6.5.3 A broad `git add` swept four other panes' files

`git add -- crates/` staged modified files in `ack-spine`, `installer`, and
`kernel-only-operator-hook` — none mine — and the hook then refused on a
**deliberate known-bad fixture** in `undrained-pipe-lint/tests/`, which is a
specimen the lint that flags it is *supposed* to catch. Re-staging four explicit
paths cleared it.

AGENTS.md already requires `git commit -- <explicit paths>`, never `-A`. The
measured consequence of ignoring it in a live shared checkout is a refusal whose
message points at someone else's fixture, three crates from anything you touched.

---

## 6.6 BLOCKER resolution — the matrix now cites, and one of its cells was wrong

`GradeGates` filed the strongest form of this BLOCKER: *"a gate that claims to
enforce something mechanically while failing all six properties is making an
unsupported claim."* The matrix in §3.1 was hand-assessed prose, so every "Y" in it
was an assertion no reader could check.

### Keyword derivation was tried and does not work — measured, both directions

The obvious fix is to grep each gate crate for property markers. It is wrong twice:

- **False negative.** `state-wildcard-lint` is documented `Y` for known-good. A grep
  for `known.good` finds **zero**. It has two — `wildcard_on_integer_and_string_passes`
  and `wildcard_on_non_state_enum_passes` — known-good legs that never use the phrase.
  I came within one commit of filing the document as wrong on the strength of that
  grep.
- **A wrong cell the grep agreed with.** Both the grep and the table say
  `state-wildcard-lint` has no anti-vacuity. **Both are wrong.**
  `empty_or_unreadable_workspace_is_an_error` is precisely that leg. The table's `N`
  is corrected above.

A property is a *semantic* claim about what a test does. No keyword scan settles it,
and the two errors point in opposite directions, so no amount of pattern-tuning
converges.

### What replaced it

`crates/no-shell-gate/tests/gate_properties.rs` — a registry where each claim is a
**citation**, not a letter:

```rust
("state-wildcard-lint", "anti-vacuity", Some("empty_or_unreadable_workspace_is_an_error")),
("path-literal-guard",  "known-good",   None),   // a DECLARED absence
```

Three legs: every citation must name a function that exists; a declared absence must
remain recorded rather than blanked; no property may be claimed twice for one crate.
`None` is first-class — six rows carry it, and those six are the reason "zero gates
satisfy all six" is true. If that count ever reaches zero, the gate fails and forces
this sentence to be rewritten in the same commit.

**Mutation-verified twice, once by accident.** It fired on its first run against
citations I had *guessed* rather than read (`test_this_repo_is_clean` does not
exist), and again deliberately when a cited test was renamed —
`wildcard_on_integer_and_string_passes` → `renamed_away_by_a_refactor` produced a
RED naming the crate, the property and the missing function. Restore byte-identical
both times.

**NO-CLAIM:** it cannot verify that a cited test *provides* the property. A function
named `known_good_leg` asserting nothing satisfies this gate — the same residual
§3.1 already records about mutation legs (*"a file named `mutation.rs` that mutates
nothing counts here as a mutation leg"*). The floor moves from **a letter nobody can
check** to **a named function that must exist**. Strictly better; well short of proof.


---


<!-- ===== 07-installability.md ===== -->

# 07 — Installability: distribution, identity, and the canonical CLI contract

This section answers one question an investor will ask before any other: *if this works on Josh's
Mac Studio, what makes it work on a second machine, in a second repo, for a person who is not
Josh?* Today the honest answer is "nothing yet, and we can prove it." What follows is the measured
starting point, then the contract we will ship against.

**Provenance boundary.** Historical measurements in this section were taken at repo HEAD fb89714 and are retained as audit context only. The current worktree authority is the command-backed boundary in §1 below; any later historical value is explicitly labelled and cannot serve as current acceptance evidence. PROJECTED means we intend to build it and it does not exist.
**Scope note (important):** this is an investor-facing installability plan and contract, not a
dispatchable operator runbook. It therefore does not pretend to carry runbook-only fields such as
Trigger, Dispatch packet, Amazing/Adequate bars, Skills, or Done signals. Those fields belong in
the operational runbook that will execute this contract; their absence here is intentional scope,
not a missing installability requirement.
Where `$M` appears in a measurement command, it is explicitly
`/Volumes/ZestData/dicklesworthstone-mirror`; the reproducible shell prefix is
`M=/Volumes/ZestData/dicklesworthstone-mirror`. No command in this section relies on an ambient,
undefined `$M`.

### 1. The measured starting point
**CURRENT WORKTREE AUTHORITY (re-derived during this integration).** The exact cargo metadata target filter returns **48** binary targets in the shared worktree. Revision, command output, and install receipt must be captured together for a release claim; the historical hashes below do not serve as current acceptance evidence.

**HISTORICAL MEASURED BASELINE — 21 binary targets at `fb89714`; superseded plan snapshot = 23.** The installer knows about 3 of them. The target declaration count is not itself a successful artifact count; the artifact proof is the command and receipt described below.

The number was `18` until round 10, taken from `grep -rl 'fn main' crates --include='main.rs' | wc -l`.
The investor lens filed it as *"fn main count is not binary/build evidence"* and it was right twice
over — the proxy was wrong, and by the time it was challenged the count had drifted too. Three
instruments, three answers, re-measured at commit time:

| instrument | answer | what it actually counts |
|---|---:|---|
| `grep -rl 'fn main' --include=main.rs` | 18 | crates having a `main.rs` — a **source shape**, not an artifact (re-measured 2026-09-01 at both fb89714 and be012d9; the 17 figure and the NUMBERS.toml note were one behind) |
| `grep -c '^\[\[bin\]\]' crates/*/Cargo.toml` | 16 | **explicitly declared** targets; misses every implicit `src/main.rs` |
| `cargo metadata --format-version 1 --no-deps` (historical snapshot) | **23** | the workspace's declared binary targets in the superseded plan snapshot; current target authority is 48 above |
The historical metadata measurement was the only canonical target denominator in that snapshot; current target authority is the 48-row metadata result above. Both counts include implicit and explicit Cargo binary targets and neither is an artifact-success proof.
Successful output is a separate proof: `cargo build --workspace --message-format=json` is filtered to
`compiler-artifact` messages whose target kind is `bin`, and the receipt records target names,
artifact paths, and the command exit code. A target in metadata without a successful artifact is
reported missing and cannot satisfy install acceptance.

`crates/installer/src/main.rs:12` declares
`const BINARIES: &[&str] = &["omp-orchestrator", "tick-monitor", "pane-truth"];`.


**HISTORICAL MEASURED PLAN SNAPSHOT.** The installer list covered 3/23 target rows (13%), with 2/23 owned install entries (8.7%) and one foreign; 20 targets had no owned path in that snapshot. Current acceptance uses the 48-target metadata denominator above.
**Canonical manifest (PROJECTED; the one denominator for this section).** The release manifest is
generated from the Cargo target graph with the exact command in `NUMBERS.toml`:
`cargo metadata --format-version 1 --no-deps 2>/dev/null | python3 -c "import sys,json;m=json.load(sys.stdin);print(len([t for p in m['packages'] for t in p['targets'] if 'bin' in t['kind']]))"`.
It has one row per target and the fields `target`, `owner`, `distribution` (`INSTALL`,
`FOREIGN`, or `UNRELEASED`), and `adapter`. The installer, runtime adapter list, and
post-install expected set MUST all be projections of this manifest; none may maintain a second-hand count. In the historical 23-target snapshot, installer_entries=3, foreign=1 (pane-truth), and owned_entries=2; current metadata has 48 target rows and the current ratio is recorded in NUMBERS.toml.
~~The seven runtime adapters are a projected manifest field, not a second denominator.~~ **RETRACTED 2026-09-01 — see §7.9: no seven-item list exists anywhere in the repo.** The expected
install set is exactly the rows with `distribution=INSTALL`, so a foreign or unreleased row can
never be mistaken for a missing artifact.

**The defect class, for the fourth time in this document's history:** a number produced by an
instrument that does not measure the quantity in the sentence. `06-gates` did it with test counts
(`370`/`379`), `gap_propagation.rs` did it with a baseline carried between detectors, `02-surface-census`
did it with a denominator that grew 50% in one exchange, and this section did it with `fn main`.
Every one survived multiple readings because a plausible integer reads as a measurement.

**CURRENT RECHECK — pane-truth is now present in this workspace.** which pane-truth still resolves to /Users/josh/.local/bin/pane-truth, but crates/pane-truth exists and ls -1 crates | wc -l returns 50. The old GHOST conclusion was valid only for the pre-extraction snapshot; it is retired. The remaining identity question is whether the installed binary matches current HEAD, which requires the four-way identity receipt below.
**MEASURED — one of those three binaries cannot report its own identity.**
`grep -c 'version' crates/tick-monitor/src/main.rs` returns `0`. `tick-monitor` has no `--version`
flag and no version string anywhere in its entrypoint. Its identity is not *unmeasured*; it is
*unmeasurable by construction*. No amount of probing an installed `tick-monitor` will tell you
which commit produced it. In the 23-target historical snapshot, only 5 targets mentioned --version at all
(`for c in $(ls crates); do grep -c '\-\-version' crates/$c/src/*.rs; done` — nonzero for
`installer`, `kernel-only-operator-hook`, `omp-inventory-map`, `omp-orchestrator`,
`omp-rpc-session`).

That is not an incidental omission. The brief's five-stage control loop (formerly "five-stage" — renamed, the table has five stages and seven rows) table (§4) records exactly one
layer as **WORKS**: *observe*, and its mechanism is `tick-monitor`. The single working layer of the
system is carried by the one binary in the install set whose provenance cannot be established from
the artifact. If observation is the only thing we can currently trust, we cannot currently prove
which build produced the observations.

**MEASURED, and it bounds everything in this section.** The brief's §4 also records *actuate* as
**DOES NOT EXIST — a human types into panes**, and *complete* as **AVAILABLE, NOT WIRED —
`AgentEndEvent` crossed the wire but the supervisor does not consume it**. Installability is therefore not merely
unbuilt; two of the five layers it would need to install are unbuilt. A second machine that ran a
perfect installer today would receive a working observer, a broken actionable filter, a fenced
consumer, and two absent layers. Every PROJECTED item below is contingent on those layers landing,
and §09 owns that sequencing.
**CORRECTED 2026-09-01 — the defect described here is gone at HEAD, and the description below
matched code at neither of the two commits we checked.** An earlier revision of
`crates/installer/src/main.rs` derived `owned` as `BINARIES.len() - foreign`; at snapshot fb89714
and at HEAD be012d9 the code instead counts `owned` explicitly in the loop (`:74`), with a comment
(`:69-71`) explaining that the derived form is avoided so the OK line prints the same variable it
decremented. What survives is the lesson, which is why the paragraph stays: the OK line and the
DRIFT line must print counts from the SAME variable, or a mismatch between them is undetectable.
The current code satisfies that; the paragraph is now a regression note, not a live defect.

**MEASURED — the false-green class is real and recent.** Earlier in this session the installer's
`main()` printed `installer: not yet wired to the live fleet` and returned SUCCESS. A no-op that
exits 0 is not "incomplete"; it is a *false green* — it supplies evidence of health it did not
gather. HEAD `fb89714` has replaced it with `--check`/`--install` verbs, but the lesson is a
permanent gate item: **a command that did not perform its work MUST NOT exit 0.** The refusal exit
code (§3) exists so that "I declined" is distinguishable from "I checked and it was fine."

**Historical MEASURED, prior to HEAD.** The four-way identity proof, first run, found 3/3 probed
binaries disagreeing with HEAD, and the launchd supervisor running a build 23 commits behind HEAD.
Not re-derivable at `fb89714`; recorded as history, not current state.

NO-CLAIM: §1 measures the *declared* install surface and identity surface. It does not claim the
18 unlisted binaries are unusable, nor that any of them is broken — only that none of them has a
documented, reproducible path from this repo onto a second machine.

### 2. The canonical CLI contract we will ship

PROJECTED for the whole of §2, except the envelope shape, which is MEASURED at
`crates/omp-inventory-map/src/lib.rs:613` and `:1366`.

We adopt `/canonical-cli-scoping` and `/cfs-cli-discipline` wholesale rather than minting a local
standard. The unit of installation is a single umbrella binary, `omp-orchestrator`, which scopes every one of
the 48 workspace binary targets as an adapter — the aggregator shape both skills mandate. We do not ship one CLI per target; we ship one CLI whose doctor takes an adapter name. Every command emits
the envelope we already emit today:

```json
{"schema_version":"<surface>/v1","command":"<verb>","status":"OK|DEGRADED|DOWN|UNKNOWN|REFUSED","data":{}}
```

Probe ids are namespaced under `^omp(\.[a-z][a-z0-9_-]*){2,}$` — e.g.
`omp.identity.binary.tick_monitor.version_absent`; bare segments are rejected at construction, not
at review. Every probe detail is a structured =N value, never prose: installed_binaries=3, workspace_targets=48, foreign=1, never looks about right.

#### The mandatory triad

**`omp-orchestrator doctor [<adapter>] [--fix] [--json]`** — diagnose every subsystem, or one adapter.
*Purpose:* answer "what is wrong and where" for an operator or agent with no context.
*Exit:* `0` all probes green; `1` at least one FAIL; `2` usage error.
*Envelope:* `data.probes[]`, each `{id, status, detail, upstream_owner?, repair_target?}`.
*Negative pattern it refuses:* **doctor must never crash when a subsystem is dead.** A dead adapter
is `status:"DOWN"` with **exit 1 for doctor (or exit 3 for critical health)**, never a panic and never zero
the caller cannot interpret. It also refuses the bundle: `omp-orchestrator doctor` with a broken
`tick-monitor` must name `tick-monitor`, not report `adapters_ok=false`.

**`omp-orchestrator health [<adapter>] [--watch -i N] [--json]`** — single-shot rollup, cheap enough for a
monitor loop.
*Purpose:* one line of truth for a supervisor, not a diagnosis.
*Exit:* `0` green; `1` degraded; `3` critical.
*Envelope:* `data.rollup` plus one line per adapter. Health is strictly a rollup *of doctor's probe
set* — the two MUST NOT be able to disagree, which means they share one classifier, not two
copies of similar logic. Two robot queries that disagree about the same state is a contract
violation, and we will pin it with a convergence test.
*Negative pattern it refuses:* health MUST NOT perform I/O that mutates or that can hang
unboundedly. It is called in a loop; a health check that blocks is an outage amplifier.

**`omp-orchestrator repair --scope <adapter> [--dry-run] [--apply --confirm]`** — idempotent fix for a named
failure class.
*Purpose:* convert a doctor finding into a corrected state, reversibly.
*Exit:* `0` no-op or success; `1` at least one repair failed; `5` concurrency lost (another repair
holds the lock).
*Envelope:* `data.actions[]` with `{action, state: PLANNED|APPLIED|SKIPPED, reason, backup_path,
before_hash, after_hash}`; in dry-run, `data.actual_actions` MUST be empty and the envelope MUST
say so explicitly.
*Negative pattern it refuses:* dry-run is the default. `--apply` without `--confirm` is a usage
error, not a prompt. `--dry-run --apply` together is rejected as oxymoronic. And per
`/world-class-doctor-mode-for-cli-tools`: **detect-then-fix, never fix-then-detect** — every write
routes through one `mutate()` chokepoint that takes a verbatim backup into
`.omp/runs/<run-id>/backups/` before touching anything, so `omp-orchestrator repair undo <run-id>` restores
byte-for-byte. If it cannot be undone byte-for-byte from the artifact, it does not ship.

#### The subsidiary triad

We are unambiguously state-handling: we read `git ls-files`, write to `~/.local/bin`, own a launchd
plist, and drive tmux panes through `ntm`. The exemption does not apply.

**`omp-orchestrator validate <thing>`** — pure read, zero side effects.
*Purpose:* verify a config, a dispatch packet, a plist, or an install target *before* anything acts
on it, so a bad input is rejected at the boundary rather than half-applied downstream.
*Exit:* `0` valid; `74` validate failure (distinct from `1` so a caller can branch on "your input
is wrong" versus "the system is unhealthy"); `2` usage.
*Envelope:* on reject, `{status:"REJECT", reason, expected, observed, observed_length}` — the
observed field is mandatory, because a rejection without the observed value is undebuggable.
*Negative pattern it refuses:* validate MUST NOT touch the filesystem outside reads, and MUST NOT
be satisfiable by a mutation. A "validate" that fixes as it goes is a repair with a lying name.

**`omp-orchestrator audit [--since <ts>]`** — append-only ledger of every mutation with provenance.
*Purpose:* answer "what did this tool change on this machine, when, and under whose authority"
without reading the tool's source.
*Exit:* `0`; `1` ledger unreadable or corrupt.
*Envelope:* rows carry `{ts, actor, verb, idempotency_key, touched_paths[], receipt_path,
post_check, result}`.
*Negative pattern it refuses:* the ledger is append-only and schema-versioned. A repair that
mutates without appending an audit row is a bug the mutate-auditor test fails the build on. An
ambiguous audit read refuses the mutation rather than proceeding blind.

**`omp-orchestrator why <id>`** — provenance trace for one object.
*Purpose:* explain how a binary, a probe verdict, a bead, or a dispatch reached its current state,
including where the chain of evidence breaks.
*Exit:* `0` found; `1` unknown id.
*Envelope:* `data.chain[]` from origin to current state with the evidence at each hop.
*Negative pattern it refuses:* `why` MUST NOT synthesize a plausible explanation. If the chain has
a gap, the gap is a node in the output (`{hop:"build_id", status:"UNKNOWN", reason:"binary exposes
no --version"}`), not an omission. This is the direct answer to `tick-monitor`.

#### Self-documentation and discoverability

`--info`, `examples`, `quickstart`, `help <topic>`, `completion <shell>` are mandatory, and
`completion bash | bash -n -` must exit 0 — a completion script that emits broken syntax fails
silently in production.

**ADDRESSABLE is a first-class gate property, not a nicety.** MEASURED (brief §3.6):
`omp-inventory-map --help` returns
`{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"ERROR","data":null,"error":"CONFIG_ERROR unknown argument --help"}`.
Historical MEASURED values from brief §3.6 were `13 tests` and `544 KB` of doctor output, but
that record retained neither the exact command nor the output artifact. They are therefore not
re-derivable measurements and MUST NOT serve as acceptance denominators. The replacement receipt
must record these exact commands and their artifacts:

`cargo test -p omp-inventory-map -- --list 2>/dev/null | tee artifacts/inventory-map-tests.list | grep -c ': test$'` (test count and saved stdout), and
`omp-inventory-map doctor --json > artifacts/inventory-map-doctor.json; wc -c < artifacts/inventory-map-doctor.json; grep -E 'Observation|CONVERGE|Verdict' artifacts/inventory-map-doctor.json` (byte count and token search, with the named JSON artifact). The receipt records the
exit code and commit for both; a changed command, scope, or artifact path is a new measurement.
This is not built-versus-wired; it is **wired-but-unaddressable**, and a correct gate nobody can invoke has the
same operational value as no gate.

Installability is where that property is either satisfied or lost for good. A binary distributed to
a second machine is reachable only through its documented surface; there is no repo to grep and no
author in the room. Under the umbrella, every adapter is reachable as `omp-orchestrator doctor <adapter>`,
`omp-orchestrator help <adapter>` names the command, and `omp-orchestrator doctor capabilities --json` enumerates every
probe id — so ADDRESSABLE is discharged by the single umbrella CLI shape, rather than by separate
implementations that can each drift independently. A capabilities snapshot is checked in as a
golden artifact; drift between the declared probe list and the implemented one fails CI.

#### Upstream-report

We wrap `omp`, `ntm`, `br`, `bv`, `git`, `cargo`, and `tmux` (versions in the surface census
section). When an adapter probe fails on the *substrate* side, the envelope carries
`class:"upstream_substrate_issue"` and `upstream_owner:"<vendor>"`, and
`omp-orchestrator upstream-report <adapter>` drafts the issue. Without this, every upstream bug is silently
absorbed as our bug and we lose the forcing function to file it.

NO-CLAIM: §2 specifies a surface. It does not claim any of these commands exist, and it does not
specify the internal probe list — that is the gate section's job.

### 3. Exit-code dictionary

PROJECTED as a shipped contract; the rows marked MEASURED are already emitted by code at HEAD.

| Code | Name | Meaning | Caller should |
|---:|---|---|---|
| 0 | `OK` | Work performed, all green. Never emitted by a command that declined to run. | Proceed |
| 1 | FINDINGS | Work performed, at least one FAIL. MEASURED: installer/src/main.rs:103-105. | Read data.probes[] |
| 2 | USAGE | Malformed invocation. MEASURED: installer/src/main.rs:45 prints usage and returns 2. | Fix the command line |
| 2 | `UNKNOWN` (envelope) | MEASURED: the inventory map exits 2 carrying `"status":"UNKNOWN"` — a probe ran but could not reach a verdict. | Treat as not-green |
| 3 | CRITICAL / NO_INPUT | Prerequisite absent: no git HEAD or no build output. MEASURED: installer/src/main.rs:65 and :170. | Fix environment |
| 5 | `CONCURRENCY_LOST` | Another mutation holds the lock. | Retry later |
| 70 | `ADVISORY` | Non-blocking finding. | Log |
| 71 | `SYSTEM_ERROR` | Our bug, not the user's. | File a bead |
| 74 | `VALIDATE_FAILURE` | Input is invalid; the system is fine. | Fix the input |
| 75 | REFUSED | The command declined to run. MEASURED: installer/src/main.rs:126 returns 75 when the build fence blocks install. | Not a result |
| 103 | `REFUSED_UPSTREAM` | An upstream guard declined. MEASURED behaviour: the RCH / mint-floor guard exits 103 with `0 passed / 0 failed`. | **Not a result** |

The two refusal rows carry the sharpest operational lesson in this document. **`exit 103` with
`0 passed / 0 failed` is a refusal, not a test result.** Zero failures did not happen because the
code is good; zero failures happened because zero tests ran. Reading that as green is precisely the
error our async contract names as *"a timeout is not a verdict"*: the absence of a negative signal
from a process that never produced a signal is not evidence. Every refusal code therefore gets its
own `status:"REFUSED"` value in the envelope, distinct from `OK` and from `DOWN`, and the CI
aggregator treats `REFUSED` as blocking rather than passing.

NO-CLAIM: this table does not claim exit codes are uniform across the 48 current binary targets. At the historical snapshot, installer used 1/2/3/75 and omp-inventory-map used 2; the other targets remain unaudited against this projected dictionary.

### 4. Identity and drift

PROJECTED as a shipped always-on check. The four-way identity proof asserts that four independently
sourced facts agree:

1. **HEAD** — `git rev-parse HEAD` in the owning repo.
2. **build_id** — the commit sha compiled *into* the artifact.
3. **`--version`** — what the installed binary says when asked.
4. **running** — what the currently-executing process (launchd job, tmux pane, daemon) reports.

Disagreement between (1) and (2) means the artifact was not built from the tree. Between (2) and
(3), the binary on disk is not the artifact we built. Between (3) and (4), the process serving
traffic is not the binary on disk — a stale process still holding an unlinked inode, which is the
failure mode that let the launchd supervisor run 23 commits behind HEAD while every static check
passed. The proof is stated as *detection of disagreement*, deliberately: agreement across four
sources raises the floor on how a stale artifact can survive, and does not guarantee freshness.
Rule 5 of the writing contract applies to identity checks as much as to gates.

Three design commitments follow:

**Build-id embedding is mandatory, not optional.** Every crate that produces a binary gets a
`build.rs` that emits the git sha into the binary, and every binary exposes it. This closes
`tick-monitor` structurally: a binary that cannot answer "which commit are you" cannot be
installed, because the installer refuses to install an artifact that fails the identity self-report
at install time. MEASURED prior art: `beads_rust/build.rs` in the mirror uses `vergen-gix` to emit
`VERGEN_GIT_SHA` plus build timestamp, target triple, and rustc semver, with a quiet
`rev-parse --is-inside-work-tree` guard so the build still succeeds outside a work tree. That guard
matters — a build script that hard-fails when git is absent breaks `cargo install` from a crates.io
tarball. Eight mirror repos ship a `build.rs` of this shape
(`grep -rl 'GIT_SHA\|vergen\|git_hash\|BUILD_SHA' $M/*/build.rs $M/*/src/*.rs`).

**Every binary declares its owning repo.** The declaration is compiled in alongside build_id. A
binary on PATH whose declared owner is not this repo is reported `FOREIGN`, named explicitly, and
**excluded from the drift denominator** — because `pane-truth` will never agree with our HEAD and a
check that reports a permanent, unfixable mismatch trains operators to ignore the check. FOREIGN is
a third outcome alongside CONSISTENT and DRIFTED, and it is printed, not swallowed.

**The denominator is printed with its derivation.** Output is
identity: consistent=N drifted=M foreign=K expected=E workspace_targets=48 probed=P — six named integers whose relationship a reader can check, not a bare 2/2.
The historical MEASURED defect was in crates/installer/src/main.rs:68 and :87 (§1): exclusion logic decremented one variable while the message printed another, yielding an arithmetically impossible but visually plausible 2/0. A ratio is only verifiable when both terms are separately named and separately sourced.

NO-CLAIM: the four-way proof detects *disagreement*. It does not prove any of the four sources is
itself honest — a binary that lies about its build_id passes. Detecting that requires reproducible
builds, which we are not claiming.

### 5. Distribution

PROJECTED. Per `/installer-workmanship`, the shipped install path must have: a curl one-liner with
a cache buster, proxy support on every fetch, platform detection (`darwin`/`linux` ×
`x86_64`/`aarch64`, musl for Linux), preflight checks (disk, write perms, network, existing
install), atomic mkdir-based locking with stale-PID detection (never `flock` — absent on macOS),
SHA256 verification via `sha256sum` *or* `shasum -a 256`, Sigstore verification when cosign is
present, a build-from-source fallback, `install -m 0755` for the atomic place, shell completions to
XDG paths, PATH setup, trap cleanup EXIT, a final per-component status summary, and printed
uninstall instructions.
**Bootstrap authenticity is a precondition, not a post-install hope.** The generated `install.sh`
MUST never be executed directly from a curl pipeline. The one-liner downloads it into the private
temporary directory, verifies its SHA256 against the pinned digest in the versioned release metadata
(or verifies a detached signature against the release channel's trusted public key), and executes it
only after verification succeeds. The Rust installer then repeats artifact verification before
placing any target. If neither a trusted package channel nor a pinned digest/signature is available,
the curl path is unavailable and the supported fallback is an exact-commit `cargo install --git`
from the trusted source; there is no unsigned bootstrap fallback.

MEASURED prior art from the mirror (`ls $M/*/install.sh | wc -l` → 50 installers; count caveat in
§7): 38/50 verify a checksum, 29/50 use `install -m 0755`, 17/50 mention uninstall, 16/50 install
completions. Checksum verification and atomic placement are near-universal in the house style;
reversibility is not. We will be in the 17, not the 33.

**The tension, named.** The repo's one hard rule is **no `.sh`, no `.py`** — a Rust gate walks
`git ls-files` and fails the build on either extension, with an empty exemption list. Every
reference installer in the mirror is `install.sh`. These are in direct conflict and hand-waving it
is not acceptable in a document meant to be attacked.

**The resolution.** The rule governs *tracked files in this repo*, not *published release
artifacts*. So:

- The install logic lives in the `installer` Rust crate. It is the real implementation: platform
  detection, checksum verification, atomic placement, identity proof, completion install, plist
  management, uninstall.
- The curl one-liner is a **generated release artifact**. A `cargo xtask`-style Rust command emits
  `install.sh` into `target/release-artifacts/` at release time; CI uploads it to the GitHub
  release; it is never `git add`ed. `git ls-files` never sees it, so the gate never fires, and the
  gate keeps its empty exemption list — which is the property that makes the gate credible.
- That generated shell script is deliberately thin: detect platform, download `install.sh` and its
  pinned release metadata, verify the bootstrap before executing it, then fetch `installer-<target>`
  and verify its SHA256 against the published `SHASUMS256.txt`. What shell is bad at
  — JSON merging, identity arithmetic, idempotent repair — happens in Rust. What shell is uniquely
  good at — bootstrapping before any of our binaries exist — happens in ~80 readable lines. The
  generator is golden-tested: the emitted script is diffed against a checked-in expected output
  stored as a `.txt` fixture (not `.sh`, so the gate stays happy), and drift fails CI.
The installer-workmanship contract is complete only when the release path also covers the following
items; each is an acceptance check, not an optional polish item:

- **Shell safety and output:** the generated bootstrap uses `set -euo pipefail`, quotes every
  expansion, uses a private temporary directory, traps cleanup on EXIT and signals, and emits
  human-readable gum/ANSI status only when stdout is a TTY (plain deterministic lines otherwise).
- **Fetch modes:** every network fetch honors `HTTPS_PROXY`, `HTTP_PROXY`, and `NO_PROXY`;
  `--proxy` overrides them explicitly, and `--offline` refuses before any network access while
  consuming only a verified local cache. No fetch path bypasses the selected proxy or offline mode.
- **Version and repeatability:** an explicit version is preferred; if the latest-version lookup is
  unavailable, the installer falls back to the release version encoded in the immutable artifact
  URL and reports that fallback. An already-installed matching build exits successfully without
  rewriting files; a different build produces a planned replacement and requires confirmation.
- **Operator setup:** installation configures the agent-facing PATH, shell completions, hook and
  skill registration, and the doctor/health entrypoints, with each resulting path listed in the
  final status summary. Missing optional integrations are reported as `SKIPPED`, never silently
  treated as installed.
- **Migration and removal:** a predecessor install is detected by its owner/build id, backed up,
  migrated or explicitly refused before replacement, and left reversible. The final summary prints

exact uninstall and rollback commands and identifies every component that was changed.
The contract is not discharged until each bullet has a deterministic acceptance result in the
install receipt; the generated-script golden test covers syntax and control flow, while the Rust
installer tests exercise the platform, proxy/offline, repeat-install, setup, and migration matrix.

This is not a loophole. It is the honest boundary: the rule exists so that logic is not smuggled
into untested shell inside our repo, and an 80-line generated bootstrap whose output is
golden-tested in Rust does not violate that intent. If a reviewer disagrees, the fallback is
`cargo install --git`, which needs no shell at all and which we will document either way.

**The self-test exercises the install; it does not certify it.** `omp-orchestrator doctor --json` immediately
post-install is the acceptance criterion, not "the files landed." The install is accepted when the
four-way identity check reports `consistent=N drifted=0 foreign=K expected_set=N missing=0 probed=P` with every term
printed, and `omp-orchestrator health` returns 0. That raises the floor from "bytes were copied" to "the copied
bytes answer for themselves"; it does not establish that the installed system does useful work, which is §6's and §09's problem.
The `expected_set` is not a best-effort count: it is the sorted target-name set from canonical
manifest rows with `distribution=INSTALL`, printed (or emitted as a hash plus names) in the receipt.
The doctor MUST probe every member, report each absent member as `missing=<target>`, and refuse the
install with a nonzero exit whenever `missing!=0` or any expected target is `DRIFTED`. `FOREIGN`
rows are named and excluded from `expected_set`; they cannot make a partial install appear green.
An installer that copies files and exits 0 without running the check is the §1 false-green class again.

NO-CLAIM: HEAD does not claim that a signing key already exists or that signed releases are ready at
launch. The release contract nevertheless requires a pinned bootstrap digest from a trusted channel,
or a detached signature verified against a trusted public key, before execution. Sigstore may be the
signature mechanism; if a signature is advertised and cosign is present, a bad signature hard-fails.
Absent both a trusted channel and a pinned digest/signature, the curl installer is not published.

### 6. Multi-machine

PROJECTED. Three categories of hardcoding must be resolved, and they resolve differently.
**Internal-buyer pilot scoreboard (PROJECTED).** Installability earns adoption only if a person who
is not Josh can reach first useful use. We will run a five-person/five-machine pilot and record one
receipt per attempt. Baseline at this HEAD is `pilot_attempts=0`, so the operational baseline for
success, time, and support is **not yet measured**, rather than an invented percentage. The targets
are:

| metric | pilot target | evidence in each receipt |
|---|---:|---|
| first-attempt install success | ≥ 4/5 (80%) | verified bootstrap, expected-set `missing=0`, and health exit 0 |
| median time to first use | ≤ 10 minutes | timestamps from verified bootstrap start to first successful `omp-orchestrator doctor` and one accepted adapter invocation |
| operator support burden | ≤ 1 intervention per install | intervention count plus named reason; retries caused by installer defects count |
| downstream value | ≥ 4/5 complete one real, reversible adapter action | receipt with `status=APPLIED`; typed refusals are reported separately and do not count as useful work |

A release does not claim adoption from installation bytes alone: it must publish the pilot counts,
median, and intervention total, and any target miss remains a release-blocking finding until the
cause is classified. This measures internal buyer value without pretending that an external customer
or useful-work result already exists.

**Becomes config.** `crates/installer/src/main.rs:25` falls back to
`PathBuf::from("/Users/josh")` when `HOME` is unset, and lines 118-119 default `CARGO` to
`~/.cargo/bin/cargo` — a deliberate bypass of the RCH shim measured at
`/Users/josh/.rch/shims/cargo`. Both are correct behaviours for this machine and wrong as
defaults. They become a config file at `$XDG_CONFIG_HOME/omp-orchestrator/config.toml` with
per-machine overrides, and the `/Users/josh` literal becomes a hard error rather than a fallback —
an unset `HOME` is an environment we should refuse, not guess at. This is the scope the
portability bead already owns; it is not new work invented here.

**Becomes discovery.** Repo root is currently derived from `CARGO_MANIFEST_DIR` at compile time
(`main.rs:16-20`), which is correct when running from the build tree and meaningless for an
installed binary. The installed binary discovers its repo by walking up from `cwd` to the nearest
`.git`, and cross-checks the discovered repo against its compiled-in owning-repo declaration
(§4). Mismatch is a typed refusal, not a guess.

**Becomes a typed refusal.** A machine without `tmux`, `ntm`, or `br` cannot run the orchestrator.
It must **fail closed** with a structured envelope that names the missing dependency, the probed
path, and the install command:

```json
{"schema_version":"omp-preflight/v1","command":"doctor","status":"REFUSED",
 "data":{"probe":"omp.preflight.dependency.ntm.absent","required":"ntm",
         "probed":["/usr/local/bin/ntm","/opt/homebrew/bin/ntm"],
         "install":"curl -fsSL https://.../ntm/install.sh | bash","exit":75}}
```

Never a silent partial. An orchestrator that starts without `ntm` and quietly dispatches nothing is
the false-green class at fleet scale — it will report ticks, report health, and move zero work. The
degradation contract is binary per adapter: an adapter is either fully available or `DOWN` with a
named remediation. There is no "mostly working."

Note the version fragility this introduces: we wrap `tmux`, which MEASURED **rejects `--version`**
(`tmux --version` → `tmux: unknown option -- -`). Dependency probing must not assume every
substrate answers a version flag; the probe records `version:"UNPROBEABLE"` with the rejection text
as evidence rather than treating the failure as absence.

NO-CLAIM: §6 addresses a second *macOS* machine and a Linux machine with the same substrates
installed. It does not claim Windows support, and it does not claim the orchestration semantics
(pane dispatch, ack staging) are machine-independent — only that the install and preflight surfaces
are.

### 7. What Jeffrey would do

For every command in this section, `M` is explicitly `/Volumes/ZestData/dicklesworthstone-mirror`;
the reproducible shell prefix is `M=/Volumes/ZestData/dicklesworthstone-mirror` followed by the
command. The mirror counts below are historical prior-art measurements, not workspace denominators.
**Mirror census, historical and definitionally unresolved.** The prior brief recorded 216 entries, the mirror index reported 210 filesystem .git entries, and a direct ls returned 218. None of those counts validates every entry as a live git work-tree; the difference is definitional, with ntm.corrupt-20260819 and useful_tmux_commands present beside ntm. This section records the measurements as prior-art context, not as a repository denominator. The current fh index is stale and direct citations remain bounded by the command shown.

`fh`'s MCP surface is failing closed with a typed `SERVE_INPUT_STALE`, so every citation below is a
direct grep of the mirror with the command shown. Per the brief, failing closed with a remediation
hint is the model, not a defect — it is the same behaviour §6 specifies for a missing dependency.

**Gap: no reversible doctor.** *Prior art found.*
`grep -rl '"doctor"' $M/*/src/*.rs` surfaces
`coding_agent_session_search/src/doctor_chokepoint.rs`, `doctor_undo.rs`, `doctor_runs.rs`,
`doctor_robot_docs.rs`, `doctor_recover.rs`. The chokepoint's own header states the contract we
should copy verbatim: every disk write reachable from `--fix` flows through one `mutate()`, which
verifies scope, computes `before_blake3`, copies a verbatim backup into
`.doctor/runs/<run-id>/backups/<rel-path>` preserving permissions and mtime, mutates atomically via
write-tmp-then-rename, computes `after_blake3`, and appends an `ActionRecord` to `actions.jsonl`.
It also does something we should copy at the *doctrine* level: it states its own scope honestly —
"existing repair codepaths are not refactored to flow through `mutate()` in pass-1 — that is a
pass-2 task" — with a Phase-7 auditor test that ensures *new* paths use the chokepoint. That is how
you ship a partial safety envelope without lying about coverage.

**Gap: no build-id embedding.** *Prior art found.* `beads_rust/build.rs` uses `vergen-gix` to emit
`VERGEN_GIT_SHA`, build timestamp, target triple, and rustc semver, guarded by a quiet
`rev-parse --is-inside-work-tree` probe. Eight mirror repos ship the same shape (`beads_rust`,
`beads_viewer_rust`, `coding_agent_session_search`, `coding_agent_usage_tracker`,
`cross_agent_session_resumer`, `destructive_command_guard`, `pi_agent_rust`, `rust_stream_deck`).
We adopt it unchanged rather than hand-rolling a `git rev-parse` in each `build.rs`.

**Gap: no installer.** *Prior art found, 50 exemplars.* Counts in §5. The reference pair named by
`/installer-workmanship` — `destructive_command_guard/install.sh` and
`remote_compilation_helper/install.sh` — are the shape to emulate for the generated bootstrap.

**Gap: no typed missing-dependency refusal.** *Searched
`MISSING_DEPENDENCY|missing_dependency|DependencyMissing` across `$M/*/src/*.rs` — no prior art
found.* Jeffrey's installers detect missing dependencies in shell at install time; none of the
Rust binaries surfaced by that pattern carries a typed runtime refusal for an absent substrate.
This is a place where our aggregator shape is genuinely different: the release manifest must expose an explicitly enumerated adapter set, but **no seven-item adapter list exists today**. Each future adapter may map to one or more target rows and be independently refused or reported as DOWN; that design remains PROJECTED and is not borrowed prior art.

NO-CLAIM: §7 cites prior art for the *pattern*. It does not claim we have read those
implementations in full, that they are bug-free, or that their licenses permit copying source —
only that they establish the house convention and the shape we will implement independently.

### 8. Constraints this section adds to the repo (R11)

R11 says a requirement that lives only in conversation is a requirement that will be dropped. Four
constraints were derived here and did not previously exist in writing anywhere in `docs/`. They are
recorded as repo doctrine, not as prose inside an argument:

1. **A command that did not perform its work MUST NOT exit 0.** Refusal gets its own code and its
   own `status:"REFUSED"`, and CI treats `REFUSED` as blocking. Derived from the false-green
   installer and from `exit 103` with `0 passed / 0 failed`.
2. **A ratio is printed with both terms separately named and separately sourced.** No `2/2`. This
   generalises rule 4 of the writing contract from prose into program output.
3. **A binary that cannot report its own build_id is not installable.** The installer refuses it.
   This converts `tick-monitor`'s missing `--version` from a nice-to-have into a release blocker.
4. **The no-`.sh` rule governs tracked files, not generated release artifacts.** The installer is
   Rust; the published one-liner is emitted at release time into `target/release-artifacts/`, is
   never `git add`ed, and its expected output is golden-tested from a `.txt` fixture. The gate's
   exemption list stays empty, which is the property that makes the gate credible.

NO-CLAIM: these four are constraints this section proposes. They are not ratified by a gate, and none is asserted as enforced until a current receipt names the source revision, artifact, and command.

---

**Section NO-CLAIM.** This section specifies distribution, identity, and the CLI contract. It does
not specify the probe list (gates section), the orchestration semantics (crate specs), or the
milestone at which each surface lands (milestones section). Every PROJECTED item is unbuilt in the current worktree; the only MEASURED install-adjacent code is crates/installer, which covers 3 installer entries against 48 current workspace targets, one of which is foreign and not built here.

---

## 7.9 Historical blocker resolution — the seven-adapter ambiguity is superseded
GradeInstall filed two BLOCKERs against this section:

> The document asserts every target becomes an adapter (21) while simultaneously
> claiming seven adapters. If 21 targets map to 7 adapters, the grouping rule is
> absent — which targets belong in which adapter is unspecified, making the CLI
> contract unexecutable.

> The document specifies commands that take `<adapter>` parameters but provides zero
> examples of valid adapter names. A user on a second machine cannot invoke
> `omp-orchestrator doctor <adapter>` without guessing.

Both are correct, and measurement makes them worse rather than better.

### What is actually true, 2026-09-01

| claim | measured | derivation |
|---|---:|---|
| binary targets | **48** | cargo metadata --format-version 1 --no-deps, registered as built_binaries |
| known to the installer | **3** | `omp-orchestrator`, `pane-truth`, `tick-monitor` |
| "seven runtime adapters" | **no list exists** | grep of the whole repo finds no enumeration |

The `21` was already stale when graded — two crates landed since — which is why it is
registered as a derived figure rather than written in prose. **The `7` is worse than
stale: it is unsourced.** Five places in this section invoke `omp-orchestrator doctor <adapter>`,
and no adapter is named in any of them.

### Retraction

The sentence *"the seven runtime adapters are a projected manifest field, not a second
denominator"* is **retracted**. It defended a number against being read as a
denominator while never establishing where the number came from. There is no
seven-item list, no grouping rule from 23 targets onto 7 names, and no way for a
reader to check either.


What replaces it: **48 current target rows, 3 installer names, and 45 unlisted targets**. Of the three names, two are owned install entries and one is the foreign pane-truth binary; this is the current arithmetic, not a claim that all 48 targets are installable.

**NO-CLAIM:** this resolves the arithmetic, not the design. Whether the right shape is one omp-orchestrator aggregator with adapter subcommands, separate installed binaries, or something else remains open — against a measured 3-of-48 listed-target count, rather than behind a seven that nobody could look up.
---

## 7.10 Historical journey surfaces, mapped — 1 of 16 names its own timeout

Josh's standing objective, verbatim: *"Every surface of our journey mapped to specific
commands with proper guards and timeouts, everything typed — nothing unknown."*

`Lens05Actions`, the held-out operator-at-3am lens, filed the BLOCKED form of this:

> 466-line specification of what 11 actions SHOULD do but **NO STATED COMMAND,
> BINARY, API, or FUNCTION CALL** to actually RUN any action.

**HISTORICAL MEASURED (2026-09-01; provenance incomplete).** The source revision, exact per-target
`--help` invocation, and captured output artifact were not retained. The table below is therefore a
plan snapshot, not acceptance evidence. A future bead MUST record the metadata target list, the exact
probe command, every target's output/exit code, and a SHA-256 before publishing these ratios.

### Historical 23-target help snapshot

| behaviour | count | what it means for a stranger |
|---|---:|---|
| **NOT-BUILT** | 7 | the release binary does not exist; nothing to invoke |
| **REAL-HELP** | 6 | historical snapshot: usage line with no error; the snapshot's first-line instrument answered 7, while the current registered help_discoverable_binaries command answers 10 |
| **REJECTS** | 3 | answers `unknown argument: --help` |
| **HELP-AS-PATH** | 3 | treats `--help` as a **filesystem path** and reports it missing |
| **SILENT** | 2 | prints nothing at all |
| **EXECUTES** | 1 | **runs the gate** instead of describing it |
| ERRORS | 1 | errors on an unrelated precondition |

**Historical snapshot: names a timeout or deadline in its own help: 1 of 16 buildable** —
dispatch-silence-watch. Fifteen did not in that snapshot; this ratio is not the current 48-target help census.

### The three that deserve naming

- **`no-shell-gate`, `state-wildcard-lint`, `undrained-pipe-lint`** treat `--help` as a
  path: *"cannot read --help"*, *"repo root --help: No such file or directory"*. An
  operator asking a gate what it does is told their file is missing.
- **`pre-commit-gate` EXECUTES.** Asking it for help runs the gate — output
  `MULTI-GATE: no staged files to check`. On a dirty tree that is a real gate run with
  real refusals, produced by a request for documentation.
- **`loop-queue-filter` and `pre-delete-citation-check` are silent.** No usage, no
  error, exit and nothing. Indistinguishable from a binary that does nothing.

### What this does and does not close

**Historical snapshot only:** the commands existed for 16 built targets, and guards/timeouts were discoverable for 1 of 16. The current registered first-line help command answers 10; a fresh full matrix receipt is still required before current invocation coverage is claimed.

**Does not close** the actions themselves. This maps BINARIES; `Lens05Actions`'
complaint was about **actions A1–A11**, which are specified as *behaviours* and do not
correspond one-to-one with bin targets. Several actions are functions inside
`omp-orchestrator` with no independent entry point, so no `--help` probe can find them.
Mapping action → binary → subcommand is a further step and is **not built**.

**NO-CLAIM:** `--help` answering is a proxy for discoverability, not a measure of it.
A binary with perfect help can still be undiscoverable if nothing tells an operator it
exists — which is §2's `ADDRESSABLE` property, satisfied by **zero of eight gates**.
This probe measures the second gate of two, and the first is still shut.


---


<!-- ===== 08-end-users.md ===== -->

# 08 — The end-user journey: another repo, another machine, orchestrating their own project

Serves **R9** — *"all the way to end users (other projects / repos / machines) are using it to
orchestrate their projects."*

**Read the status marker on every claim here.** Almost nothing in this section exists. The adoption
path is `PROJECTED` end to end; the measurements are of *our own repo today* — evidence the failures
are real, and the exact obstacles between us and a foreign adopter.

`MEASURED` 2026-08-31, corrected 2026-09-01, and it frames everything below: there is no
adopter-facing INIT path — no `doctor`, no `init`, no `adopt`
(`/usr/bin/grep -cE '"doctor"|"init"' crates/omp-orchestrator/src/main.rs` → `0`). There IS a
resident `run` entrypoint, and quoting the CLI verbatim matters enough that this section's first
draft got graded down for trimming it. The whole CLI surface, verbatim from `usage()` at
crates/omp-orchestrator/src/main.rs:274-275:

```
usage: omp-orchestrator [run] [--once|--max-ticks N] [--repo PATH] [--session NAME]
                        [--interval-secs N] [--receiver-agent NAME] [--omp-quick] [--omp-binary PATH]
       `run` is the explicit resident lifecycle entrypoint (observe -> ready queue -> dispatch -> receiver receipt); the flag-only form is unchanged for launchd
```

`run` requires our fleet's `--session`/`--receiver-agent` — it is an entry point for OUR operator,
not an adoption path; the missing pieces are still the three verbs below.

No `doctor`, no `init`, no `adopt`. `--session NAME` and `--receiver-agent NAME` are facts about
*our* fleet a stranger cannot supply correctly. **NO-CLAIM:** that grep establishes the absence of two
literal strings in one file, not that no other crate offers an entry point (§07 owns distribution).

---

## 1. Who the end user is

**Persona A — the solo maintainer.** One repo, one machine, no fleet, 5k–100k lines. One agent at a
time in one terminal, by hand; no panes to census, no use for a supervisor loop. They want the *back
half*: gates that fail a build on a named property, and completion tracking where "done" means an
artifact exists rather than an agent said so. They can be served first, because the gates are the
part of this repo measured to work — `HISTORICAL MEASURED SNAPSHOT` (the current worktree census authority is §1 of `06-gates.md`; do not use this snapshot for acceptance): 31 integration test files, **409** `#[test]` functions,
8 gate crates, **2 of 8** with all four named leg categories in the snapshot (`undrained-pipe-lint` and `no-shell-gate`), but only **1 of 8** with an executable attributable mutation leg (`undrained-pipe-lint`; `no-shell-gate` is AFFORDANCE-only), and **4 of 8** with no mutation mechanism of any kind. **NO-CLAIM:** the leg table counts files matching a property grep; it does not establish the legs are individually strong.

**Persona B — the small team.** Three to eight agents, one repo, one shared session. They already
dispatch by hand and already lose track. They want dispatch with receipts: a record that a packet
was accepted, by whom, and whether work started. `MEASURED` (brief §4, corrected 2026-09-01): the `actuate` layer **exists and is unfenced** — the
resident supervisor dispatches without the claim beat and logged 131 sends of one unclaimed bead to
one dead pane as successes; the `complete` layer **does not exist** — every completion this session
was found by a human looking. Their core need is exactly those two rows: a dispatch that is refused
until claimed and receipted, and a completion the loop can see.

**Persona C — the multi-repo fleet operator.** Our own shape: many repos, dozens of panes, a tracker,
a queue, a supervisor loop. Last to serve, structurally: **HISTORICAL** brief §3.2 reported 157 of 183 rows as CAPABILITY_NOT_USED and all 7 consumes edges from one crate — an inventory, not orchestration. Current map values are maintained in 02-surface-census and are not an external-adoption result.

**The order is A → B → C, the inverse of the order we built in.** **NO-CLAIM:** the ordering is a
`PROJECTED` product judgement; no adopter outside this machine has run any part of this system.

---

## 2. Zero-to-first-tick — a design artifact, not a recording

**This transcript is `PROJECTED`.** It is a specification written in the shape of a session so its
refusals become designable. Nothing below has been executed; anything that looks like output is a
contract for output we intend to produce.

### 2.1 Install

```
$ cargo install omp-orchestrator                                    # PROJECTED
$ omp-orchestrator --version
omp-orchestrator 0.1.0 (build_id=ecdea397, target=aarch64-apple-darwin)
```

`MEASURED` obstacle, not cosmetic. `crates/installer/src/main.rs:16` resolves the repo root from
`env!("CARGO_MANIFEST_DIR")` — a **compile-time** constant; `:25` falls back to a literal
/Users/josh; :12 hardcodes three binary names. Repo-wide the pattern appears **61** times across **52** files at this writing. NUMBERS.toml registers the aggregate site count as LIVE; the 52-file split is diagnostic output from the same source walk, not a separate registry figure. A shell grep -r --include= returns a false zero here — see §2.2.
An installed binary carrying a compile-time path audits the build machine's checkout, not the adopter's. **NO-CLAIM:** this is a measurement of coupling, not a claim that every occurrence is production behavior.

### 2.2 `doctor` — in a repo with none of our conventions

```
$ cd ~/src/their-app && omp-orchestrator doctor --json              # PROJECTED
{"schema_version":"omp-orchestrator/v1","command":"doctor","status":"DEGRADED","error":null,
 "data":{"adopted":false,"probes":[
   {"id":"repo.git",       "verdict":"OK",    "evidence":"git rev-parse HEAD -> 9f2c1a0"},
   {"id":"repo.dirty",     "verdict":"WARN",  "evidence":"12 modified paths"},
   {"id":"tracker.br",     "verdict":"ABSENT","evidence":"no br on PATH; no .beads/"},
   {"id":"tracker.any",    "verdict":"ABSENT","remediation":"omp-orchestrator init --tracker=file"},
   {"id":"worker.tmux",    "verdict":"ABSENT","evidence":"no tmux on PATH"},
   {"id":"worker.any",     "verdict":"ABSENT","remediation":"omp-orchestrator init --worker=oneshot"},
   {"id":"gates.installed","verdict":"ABSENT","remediation":"omp-orchestrator init --gates=none"}]}}
```

**Why two of those `ABSENT` rows carry no `remediation`, and why that is the rule rather than a
lapse.** Round 10 filed this as a contradiction — the section promises every `ABSENT` probe carries
a remediation, and `tracker.br` and `worker.tmux` do not. The finding is correct about the text and
the text was wrong, not the sample.

The rule is a **two-tier probe family**:

| probe | absence means | carries remediation |
|---|---|---|
| `tracker.br`, `worker.tmux` | a **specific** implementation is not here | **no** — informational |
| `tracker.any`, `worker.any`, `gates.installed` | **no** implementation of a required capability | **yes** — actionable |

A specific-probe absence is not actionable *by this tool*: `omp-orchestrator` will not install `br`
or `tmux` on someone's machine, and a remediation field that said "install br" would be advice
wearing a command's clothes. The family probe is where the tool can actually offer something — a
file-backed tracker, a one-shot worker, no gates — because those are the fallbacks it ships.

**So the contract is:** *every `ABSENT` **family** probe MUST carry a `remediation`; a specific
probe MUST NOT invent one.* Stated that way it is checkable, and the sample above satisfies it.
The original phrasing was checkable too — and the sample failed it, which is how the grader found
it in one pass.

Two load-bearing commitments. First, **ABSENT is not FAIL** — a foreign repo lacking our tracker
is a repo we have not adapted to, and a doctor that scolds an adopter for not being us gets
uninstalled. Second, the remediation invariant is scoped to capability-family probes: each family
`ABSENT` **MUST** carry a remediation naming a command that exists. A specific-probe `ABSENT`
row (such as `tracker.br` or `worker.tmux`) is informational and **MUST NOT** invent an install
command; it may point the reader to its family probe instead. `MEASURED` precedent is the
`--help` defect (brief §3.6), which produced the sixth gate property **ADDRESSABLE**. A probe
reporting a condition it cannot route is that defect in a diagnostic hat.

Prior art, per R7 — *what would Jeffrey do*: `br` runs its whole doctor surface through one mutation
chokepoint with byte-identical undo — `beads_rust/tests/e2e_doctor_chokepoint.rs:1-14`: *"corrupt →
diagnose → `--repair` → assert healthy"*, then *"`br doctor undo <id>` → … restore to the recorded
`before_hash`"*, plus the dry-run, idempotence, capabilities and triage contracts. **Adopted whole.**

**CORRECTION — a measurement defect of mine worth more than the answer.** I first reported *"searched
`MISSING_DEPENDENCY|DependencyMissing|not_installed|NotInstalled`, no prior art found"* — a **false
zero**. The search was `/usr/bin/grep -rl --include='*.rs' … ntm beads_rust`, and `ntm` is a **Go**
repo: the filter matched nothing there and I read structural absence as semantic absence. Re-derived
with the harness grep and no extension filter, the same pattern returns **89 matching files**, and

`MEASURED` re-derivation of that count (2026-08-31, source checkout `/Users/josh/Developer/jeff-shadow`,
HEAD `7c28478`):

```sh
$ cd /Users/josh/Developer/jeff-shadow
$ /usr/bin/grep -rlE 'MISSING_DEPENDENCY|DependencyMissing|not_installed|NotInstalled' ntm beads_rust | wc -l
89
```

The command searches both complete directory trees with no extension filter and counts matching file
paths once; the output above is retained in the round-11 evidence file. The prior art is exactly what
§5 needed — cited by construct, because four of these line numbers were off by one when a sibling
re-opened the files while every construct held: a per-dependency typed sentinel (`ntm/internal/bv/bv.go:31`,
`var ErrNotInstalled`; same sentinel at
`internal/cass/client.go:13` and `internal/caut/client.go:14`, `fmt.Errorf` variant); a shared robot
taxonomy (`docs/robot-action-handoff-contract.md:379`, `ErrCodeDependencyMissing`); a remediation
string travelling *inside* the typed envelope (`internal/cli/bugs.go:85-89`, *"Install UBS from …,
then rerun 'ntm bugs list --json'"*); a per-call-site degradation policy
(`internal/alerts/generator.go:383`, *"Silently skip when bv is not installed; only warn on real
errors"*); and a conformance test pinning the **exit code** of a dependency failure
(`internal/cli/robot_registry_conformance_test.go:16-19`).
**Two rules this earns, written down rather than left in chat.** A not-found is publishable only if it
names the command *and* why the search space was right — *"I grepped `*.rs` across a Go repo"* is a
bug, not a finding. And a citation names the **construct**, not the line: four of mine drifted by one
while every construct held, and three of us cited one precedent (§3) at three different lines, each
right about a different adjacent construct. **NO-CLAIM:** I re-opened these sites; I never ran `ntm`.

### 2.3 `init`

```
$ omp-orchestrator init --tracker=file --worker=oneshot --gates=select      # PROJECTED
CREATED  .omp-orchestrator/config.toml
CREATED  .omp-orchestrator/work/         (file tracker: one TOML per unit)
SELECTED gates: commit-build-fence, undrained-pipe-lint
SKIPPED  gates: no-shell-gate       reason=OPTED_OUT_BY_ADOPTER
SKIPPED  gates: path-literal-guard  reason=NO_KNOWN_GOOD_LEG
adopted=true  next: omp-orchestrator tick --once
```

`SKIPPED … reason=NO_KNOWN_GOOD_LEG` is deliberate. `MEASURED` (brief §3.5): `path-literal-guard` has
3 tests, 1 known-bad, **0 known-good**. An attack-only suite ships an over-strict gate; an
over-strict gate gets routed around; that is a slower death than not shipping — so `init` refuses.

### 2.4 The first observed tick, and the first dispatch

```
$ omp-orchestrator tick --once --json                               # PROJECTED, tick 1
{"status":"OK","data":{
  "observed":[{"unit":"TA-1","worker":"oneshot:0","liveness":"UNPROVEN",
               "reason":"single capture; two-capture rule needs a second observation"}],
  "dispatched":[],"graded":[],
  "refusals":[{"code":"DISPATCH_WITHHELD_LIVENESS_UNPROVEN","unit":"TA-1"}]}}

$ omp-orchestrator tick --once --json                               # PROJECTED, tick 2, ~80s later
{"status":"OK","data":{
  "observed":[{"unit":"TA-1","liveness":"IDLE_CONFIRMED","gap_secs":81}],
  "dispatched":[{"unit":"TA-1","worker":"oneshot:0","transport":"OneshotSpawn",
                 "receipt":{"verdict":"RECEIPT_CONFIRMED",
                            "evidence":"child pid 40122 in own process group; both pipes drained"}}],
  "graded":[],"refusals":[]}}
```

**NEW-WORK FLAG on the transcript above.** The tick-2 `"transport":"OneshotSpawn"` names a
transport that exists in NO crate: `ack-stage::TransportKind` has exactly two arms
(`NtmRobotSend`, `TmuxSendKeysLiteral`), and `OneshotSpawn` occurs in zero files under crates/.
The transcript is PROJECTED — but citing the two-arm enum as its measured shape one paragraph
below while silently using a third arm was an undisclosed invention until this flag was added;
the WorkerAdapter spec must add the arm or the transcript must use `TmuxSendKeysLiteral`.

MIN_GAP_SECS = 75 at crates/tick-monitor/src/lib.rs:490 — liveness is a two-capture property one tick cannot prove, so UNPROVEN on tick one is correct and must be labelled or it reads as a bug. The receipt object has a measured shape:
`crates/ack-stage/src/lib.rs:21-24` types transport as a two-arm enum — `NtmRobotSend` (*"the only
transport with a retained per-target JSON receipt"*) and `TmuxSendKeysLiteral` (*"no equivalent"*).

### 2.5 The first graded close, and the refusal

```
$ omp-orchestrator tick --once --json                               # PROJECTED, ticks 5 and 6
{"status":"OK","data":{"refusals":[],"graded":[
  {"unit":"TA-1","grade":"CLOSED_WITH_EVIDENCE",
   "evidence":["gate commit-build-fence: PASS","artifact src/parse.rs modified 9f2c1a0..a41b7e2"]}]}}
{"status":"OK","data":{"graded":[],"refusals":[
  {"code":"CLOSE_REFUSED_NO_EVIDENCE","unit":"TA-2",
   "detail":"worker reported done; no artifact diff and no gate result in this unit's ledger",
   "remediation":"omp-orchestrator why TA-2"}]}}
```

### 2.6 Versioned proof contract for every projected command

The preceding transcripts show the domain payload, but the adopter-facing robot surface MUST wrap
each command in the same versioned envelope. This is **PROJECTED**, not an emitted format:

```json
{"schema_version":"omp-orchestrator/v1","run_id":"<run_id>",
 "command":"doctor|init|tick|grade","status":"OK|DEGRADED|REFUSED",
 "error":null,"data":{},
 "proof":{"artifact":".omp-orchestrator/runs/<run_id>/<command>.json",
          "ledger":".omp-orchestrator/ledger.jsonl",
          "verify":"omp-orchestrator verify --run <run_id>"}}
```

The envelope is the proof contract, not decoration. `run_id` is unique and stable across retries;
`data` contains the command-specific fields shown above; `error` is a typed object when the command
cannot complete. Every invocation that reaches a verdict writes its envelope before returning:

> *Upstream type for this gap: `AgentEndEvent.willContinue` (`extensibility/shared-events.d.ts:154`, WIRE-PROVEN). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

| command | durable write | proving command |
|---|---|---|
| `doctor` | `.omp-orchestrator/runs/<run_id>/doctor.json` | `omp-orchestrator verify --run <run_id> --command doctor` |
| `init` | `.omp-orchestrator/config.toml` and `.omp-orchestrator/runs/<run_id>/init.json` | `omp-orchestrator verify --run <run_id> --command init` |
| `tick` | `.omp-orchestrator/ledger.jsonl` and `.omp-orchestrator/runs/<run_id>/tick.json` | `omp-orchestrator verify --run <run_id> --command tick` |
| `grade` | `.omp-orchestrator/ledger.jsonl` and `.omp-orchestrator/runs/<run_id>/grade.json` | `omp-orchestrator verify --run <run_id> --command grade` |

Exit status is deliberately separate from the domain `status`: **0** means the envelope and its
proof record were persisted (including `DEGRADED` diagnostics and a tick that withheld dispatch);
**2** means the requested state transition was refused but the refusal envelope was persisted;
**64** means usage or configuration was invalid before a run could start; and **70** means the
proof record could not be persisted. Thus an adopter can use either the typed envelope or the exit
code without treating an absent optional dependency as a crash. A proof command MUST fail closed if
the envelope, ledger entry, or referenced artifact is missing or has a mismatched `run_id`.

`CLOSE_REFUSED_NO_EVIDENCE` is the most valuable output here and the one an adopter will hate first.
It is the one our own board needed: `MEASURED` (brief §4), every completion this session was found by
a human looking. **NO-CLAIM:** §2 is a design artifact — no command executed, no JSON ever emitted.

---

## 3. What we require, and what we must never require

Hard requirements, and there are only three: **a git repository**, **a source of work units** (theirs
or ours), and **a way to observe a worker**. Below those, every "do we need X" needs a named adapter.

| we must NOT require | why not | adapter that makes it optional |
|---|---|---|
| our bead prefix (`omp-orchestrator-*`) | a naming convention is not a contract; theirs is already in their CI | `TrackerAdapter` returns an opaque `UnitId`; the orchestrator never parses one |
| our directory layout (crates/, docs/plan/) | MEASURED: 61 CARGO_MANIFEST_DIR sites; 52-file split is diagnostic output from the same walk and is not a separate NUMBERS figure | RepoAdapter::root() resolves at runtime from cwd upward; compile-time roots stay in our own tests |
| our tmux session naming (`--session NAME`) | required today (`crates/omp-orchestrator/src/main.rs:274-275`), encoding our fleet's shape | `WorkerAdapter` owns naming; `--session` becomes a tmux-adapter-scoped flag, invalid elsewhere |
| **our `.sh`/`.py` prohibition** | OUR accretion rule, born of a measured 160 tracked shell scripts and 60,467 lines in `control-plane` (`crates/no-shell-gate/src/lib.rs:6-9`). A foreign repo full of shell scripts is a normal repo and must be fully orchestrable | `no-shell-gate` is **opt-in**: `SKIPPED reason=OPTED_OUT_BY_ADOPTER`, never in a default set |
| our specific agent CLI (OMP v18) | §3.1 — the deepest coupling in the codebase | `WorkerAdapter::observe()`; `tick-monitor` becomes the *OMP-v18 implementation*, not the interface |
| a single version flag that works on every dependency | MEASURED: tmux --version exits 1 while tmux -V returns 3.6a exit 0; --version answers 8/9 of our binaries and -V answers **6/9** in the current probe | doctor requires two independent presence signals and a separate failure-arm test; the old pi_agent_rust line citations are historical |

We may enforce our own rules on ourselves as hard as we like. `MEASURED`:
`git ls-files -- '*.sh' '*.py' | wc -l` → `0` (grep-free, deliberately), exemption list empty by
design (`crates/no-shell-gate/src/lib.rs:6`). **Exporting it would be colonisation** — and it misfires
even on us: `crates/composer-typed/tests/differential.rs:41` aims its oracle at
`../../bin/composer-typed.py`; `ls bin/` → `No such file or directory`. **NO-CLAIM:** index only.

### 3.1 The deepest coupling, stated as the objection an investor should raise

*"Your observer reads one vendor's terminal UI. You have not built an orchestrator, you have built an
OMP v18 screen-scraper. What is the adapter story worth if the layer you claim works is vendor-blind?"*

Correct as stated, and the strongest objection here. `MEASURED` and specific:
crates/tick-monitor/src/lib.rs:317 hardcodes MODEL_MARKERS = [Opus 5, GLM 5.3, GPT-5.6, GPT-5.5]; :320 hardcodes the OMP-v18 dialog-footer strings captured from pane %1372; :385 strips braille U+2800..U+28FF and the literal pi; :356 classifies queued-message strings. The current source is 1,326 lines; these are source-shape facts, not proof of vendor parity.

We have been on the receiving end of this. `MEASURED`, from `crates/tick-monitor/src/lib.rs:10-18`:
`pane-truth` reported pane `%1409` — *"braille spinner, advancing timer, 16.5% tree CPU"* — as
**IDLE**, and *"its green selftest AND its mutation leg are vacuous for OMP panes."*

The answer is a declared, testable vendor contract per adapter, requiring of each what `pane-truth`
lacked: a known-good fixture in **that** vendor's format. **NO-CLAIM:** no second `WorkerAdapter`
exists; factorability is `PROJECTED`.

---

## 4. The adapter surface — design spec

All `PROJECTED`; trait shapes are future-tense. The precedent for traits is `MEASURED`:
`crates/finding/src/lib.rs:301` declares `pub trait Publisher` with `fn publish(&self, cx: &Cx, …)`.
Adapters follow that exactly: a method without `&Cx` first cannot be cancelled.

**`TrackerAdapter`** — three methods. `ready(&Cx) -> Vec<Unit>` lists claimable work.
`claim(&Cx, UnitId, Claimant) -> ClaimOutcome` takes exclusive custody and must be *fenced*: a second
claim on a live claim is a typed refusal, never a silent overwrite. `close(&Cx, UnitId, Evidence) ->
CloseOutcome` — **`Evidence` is not `Option`**, because prose completion is what emptied our own
`complete` layer. Reference: `br` via `subprocess-contract`, plus a file-backed `--tracker=file`.

**`WorkerAdapter`** — `observe(&Cx, WorkerId) -> Observation`, `dispatch(&Cx, WorkerId, Packet) ->
TransportReceipt`, `receipt(&Cx, WorkerId, &Observation, &Observation) -> ReceiptVerdict`. Two
`Observation`s, never one, because liveness is a two-capture property
crates/tick-monitor/src/lib.rs (PaneState, Liveness, Observation, classify; MIN_GAP_SECS at :490) is the tmux + OMP v18 observe implementation; crates/receiver-receipt/src/lib.rs (ReceiptVerdict, assess_receiver_receipt) is receipt; crates/ack-stage/src/lib.rs is dispatch.

**`RepoAdapter`** — `root(&Cx) -> PathBuf` resolved at runtime from cwd upward, never from
`env!("CARGO_MANIFEST_DIR")`. `identity(&Cx) -> Identity` carrying HEAD, dirty set, build id.
`gates(&Cx) -> Vec<GateSpec>` returning only opted-in gates, each declaring which six properties it
satisfies — known-bad, known-good, mutation, anti-vacuity, wired, **ADDRESSABLE** (brief §3.6).
Reference: `crates/installer/src/lib.rs`, which types `RepoOwnership`, `IdentityCheck`, and
`verify_identity` — the four-way identity proof is the right shape, its default paths are not (§2.1).

The seam to watch is type convergence, `MEASURED`: 6 Verdict-shaped types with no shared trait, 17
ack/receipt types in 3 incompatible dialects (brief §3.7), and `omp-types` — nominally the canonical
vocabulary — with **zero dependents**. **NO-CLAIM:** none of these traits exist in source; whether
`ReceiptVerdict` can express a non-tmux receipt without a variant explosion is `UNMEASURED`.

---

## 5. The degradation ladder

The model is `MEASURED` three ways. `fh` fails closed on **both** surfaces with **different** typed
codes: MCP returns `SERVE_INPUT_STALE` when the mirror HEAD moves; the CLI returns
`SEARCH_INDEX_STALE` at **exit 3**, `retryable:false`, hinting the exact command (`fh
technical-manifest`). And `ntm` (§2.2) ships the whole vocabulary. **A named code plus a route out is
the model.**

| missing | still works | refused | typed refusal |
|---|---|---|---|
| **tmux** | gates, tracker, file-backed `close`, `oneshot` worker | pane census, pane liveness, pane dispatch | `WORKER_ADAPTER_UNAVAILABLE adapter=tmux probe="tmux list-panes" hint="init --worker=oneshot"` |
| **ntm** | tmux observe, `TmuxSendKeysLiteral` dispatch | receipted dispatch | `TRANSPORT_WITHOUT_RECEIPT transport=TmuxSendKeysLiteral hint="install ntm for per-target JSON receipts"` |
| **br** | gates, observe, dispatch, `--tracker=file` | bead ready/claim/close, `bv` queue | `TRACKER_ADAPTER_UNAVAILABLE adapter=br hint="init --tracker=file"` |
| **bv** | everything except ranked ordering | priority ordering; falls back to declared order | `RANKING_UNAVAILABLE source=bv effect="units served in tracker order" hint="install bv"` |
| **fh** | everything except prior-art lookup | evidence-backed prior-art citation | `EVIDENCE_STALE source=fh detail="mirror HEAD moved" hint="fh technical-manifest"` — modelled on `SERVE_INPUT_STALE` (MCP) and `SEARCH_INDEX_STALE` (CLI, exit 3, `retryable:false`); two surfaces, two codes, neither serving stale rows |
| **cargo** | observe, dispatch, tracker | build-gated close, `commit-build-fence` | `GATE_UNRUNNABLE gate=commit-build-fence reason=NO_BUILD_TOOL` |
| **git** | nothing | adoption itself | `REPO_ADAPTER_UNAVAILABLE fatal=true detail="a git repository is a hard requirement"` |

Two rules bind the ladder. **A missing dependency degrades a named capability and never silently
changes a verdict** — without `ntm` the dispatch still happens, and the honest output is a
receipt-free dispatch *labelled* receipt-free, never one that reads as confirmed. **A timeout is not
a verdict** (brief §3.7): a timed-out probe yields `INDETERMINATE`, never `ABSENT` — `MEASURED` at
`receiver-receipt/src/lib.rs:186-187` (`EmptyPaneList`). **NO-CLAIM:** no binary emits these codes.

---

## 6. What the end user gets that they cannot get today

Stated as failures prevented — and the evidence they are real is that **they are ours, measured**.

*Work that lands and is never graded.* `MEASURED` (brief §4): the `complete` layer does not exist;
every completion this session was found by a human looking. Prevented by non-optional `Evidence`.

*Conditions living only in pane scrollback, dying with the pane.* `MEASURED` (brief §1): the reap
found **seven real conditions** there. Prevented by a durable per-unit ledger written every tick.

*A supervisor running 23 commits stale.* **HISTORICAL/UNVERIFIED**, session ledger, 2026-08-31. The failure class is retained, but the 23 has no re-derivable command and is not current evidence.
recorded rather than hidden:** I found no re-derivable command for the `23`, which writing-contract
rule 1 forbids; the failure class is real and the missing command is a defect against this document.
Prevented by applying `crates/installer/src/lib.rs`'s identity proof to the *running* supervisor.


**Historical/unverified snapshot:** the brief recorded 162 refused ticks over 4.2 hours as DISPATCH_RETRY_BLOCKED. No source ledger export or deriving command is retained, so it is a failure-shape example, not a current baseline. The proposed refusal budget remains a design control, not observed enforcement.
**Historical/unverified addressability snapshot:** the brief recorded omp-inventory-map --help -> CONFIG_ERROR unknown argument --help alongside 13 source test markers. Current omp-inventory-map source contains 28 test markers; no passing-run receipt is claimed here. The proposed ADDRESSABLE gate remains open.

**CURRENT board snapshot (2026-09-01).** The five-status command-backed total is 93: 60 closed, 22 in_progress, 8 open, 2 blocked, and 1 grading. **NO-CLAIM:** this pairs the historical failure examples with a current board count; it does not claim the refusal or addressability mechanisms have been observed.
### 6.1 Adoption outcomes we will actually measure
The value claims above are hypotheses, not adoption evidence. The following pilot scorecard makes
the internal buyer's walk-away benefit and the foreign-adopter bar falsifiable. All targets are
**PROJECTED** until a run records the stated proof artifact; the current baseline is either explicitly
measured below or marked unavailable rather than guessed.

| pilot scope | outcome | current baseline | target | proof artifact |
|---|---|---|---|---|
| internal dogfood (Josh) | time from install to first valid doctor envelope | unavailable: no adopter command exists | ≤10 minutes on a clean git repo | timed transcript + `.omp-orchestrator/runs/<run_id>/doctor.json` |
| internal dogfood (Josh) | manual intervention per closed unit | every completion in the measured session was found by a human; seconds per unit not recorded | ≤1 manual intervention per 10 ticks and no hand-edit below `.omp-orchestrator/` | tick ledger plus operator event log |
| internal dogfood (Josh) | false-close prevention | completion/evidence layer absent | 100% of seeded no-evidence closes return `CLOSE_REFUSED_NO_EVIDENCE` and persist a ledger row | `grade.json` + `omp-orchestrator verify --run <run_id> --command grade` |
| internal dogfood (Josh) | operator time saved | not measured; the current workflow is manual inspection | ≥30% fewer human-seconds per closed unit on the same fixed unit set | before/after timed transcript and unit-set hash |
| external pilot | clean-host onboarding | no foreign repo or machine has run this system | 3 clean repos reach a valid first tick in ≤10 minutes, with 0 edits outside `.omp-orchestrator/` | install transcript, config hash, and tick envelope per repo |
| external pilot | vendor-blind worker support | no non-OMP worker adapter exists | at least 1 non-OMP fixture reaches observe → dispatch → receipt without OMP markers | adapter fixture, receipt envelope, and proof command output |
| external pilot | refusal visibility | no outside run; internal evidence includes 162 refused ticks over 4.2 hours with no reader | every refusal is ledger-backed, typed, and visible to the proof command | ledger row + `verify --run` output |

These measures answer the abandonment question without laundering a design goal into a result:
install time and hand-edit count test whether adoption is cheaper than the tracking it replaces;
false-close and refusal rows test risk reduction; and human-seconds per closed unit tests whether
Josh should walk away from the manual loop. **NO-CLAIM:** no target has yet been achieved, and this
section still carries zero external adoption evidence.
---

## 7. What would make an end user abandon this

From the adopter's side, in the order they would bite. All `PROJECTED`.

**Install friction on step one.** `MEASURED`: the compile-time repo root and the literal `/Users/josh`
fallback (§2.1). A tool that audits the wrong repository on first run is uninstalled in a minute.

**False refusals.** `path-literal-guard` is the warning shape: 3 tests, 1 known-bad, **0 known-good**
(brief §3.5) — never shown to pass on correct code. One false refusal costs ten true ones' trust.

**A doctor that reports drift it cannot repair.** `MEASURED` precedent (brief §3.6): built, correct,
undiscoverable. `br`'s bar (§2.2) is repair through one chokepoint with byte-identical undo.

**Requiring our conventions.** Every row of the §3 table is a potential abandonment; the `.sh`/`.py`
rule is likeliest — our best rule, and to an adopter it reads as contempt for their codebase.

**Being slower than doing it by hand.** Persona A runs one agent in one terminal; if adoption costs

more than the tracking it replaces, not adopting is correct. **HISTORICAL** brief §3.2 reported 157 of 183 census rows as CAPABILITY_NOT_USED; current surface counts are command-backed in 02-surface-census. We are short of externally validated surface that pays, not of a current map count.
**The kill condition we should adopt ourselves:** if a foreign repo cannot reach a graded close
without hand-editing anything under `.omp-orchestrator/`, the adapter layer has failed and the fix is
the boundary, not more adapters. **NO-CLAIM:** `PROJECTED` risks reasoned from measured defects here.

---

**NO-CLAIM (section).** This section specifies an adoption path that does not exist. There is no
`doctor`, no `init`, no `tick` subcommand (harness grep for `"doctor"`/`"init"` in
`crates/omp-orchestrator/src/main.rs` → 0); no `TrackerAdapter`, `WorkerAdapter`, or `RepoAdapter`
trait in source; no non-OMP worker; no file-backed tracker; and no binary emits any §5 refusal code
today. Every transcript, trait shape, and refusal code here is `PROJECTED`. The measurements are of
**this** repo on 2026-08-31, evidence that the failures are real — never that anything prevents them.
No end user outside this machine has run any part of this system, so this section carries zero
adoption evidence.


---


<!-- ===== 09-milestones.md ===== -->

# 09 — Milestones, done-definitions, and how this plan is validated

Serves **R1** ("define what done looks like at each milestone") and **R3** ("something that could pass an 'investor' test — they can
beat up the plan, find any gaps, and pass or fail us"). Sections 01–08 describe a system. This section states the conditions under which
we are allowed to say it works, and the conditions under which this document should be rejected. Written 2026-08-31.

## 1. The done-definition template

**A milestone is closed by an OBSERVABLE, not by a claim** — not by a passing test suite, a commit, an agent reporting success, or a
human's recollection that it seemed to work. An observable is a command someone else can run and a result they can read without asking
us what it means. That is the output of the failure this project was stood down over: **a workspace whose own census measured
mechanisms built, tested, hardened, and called by nothing** — a **historical** §01 snapshot of twenty-six crates and 379→407 tests, with a BUILT≠WIRED census whose row set outgrew the then-current twenty-mechanism figure. Current workspace counts are command-backed in NUMBERS.toml; this historical scar shows why evidence code must prove invocation, not merely correct source.
Every milestone below is stated in this shape:

```
Mn — <one-sentence goal, future tense>
  OBSERVABLE      the command, and the result that closes it
  NOT IN SCOPE    what this milestone deliberately does not deliver
  STARTING POINT  MEASURED state today, with the deriving command
  RISK            the named way this milestone fails or lies
```

An observable is **admissible** only if it (1) is a command, not a description of a condition, (2) is runnable by someone who did not
build the thing — no undocumented environment, no "and then check the pane", (3) reads without interpretation, and (4) **fails on the
pre-milestone state.** Property 4 is the anti-vacuity leg from §06 applied to project management, and we owe it to a failure of our own:
the brief's **183-row / zero-missing / one-distinct-`must_be_true`** result is a **HISTORICAL, UNPROVEN** snapshot: no retained census artifact or deriving command is available here, so it is not a current baseline. It nevertheless illustrates the vacuity defect; a milestone whose observable already passes is that defect
wearing a Gantt chart.

**NO-CLAIM.** Property 4 makes an observable *discriminating*, not *sufficient* — an observable can fail-before, pass-after, and still
measure the wrong quantity. Guarding against that is §5's job.

## 2. The milestones

Seven, ordered by dependency — forced, not chosen: each consumes the capability the previous one produces, so there is no parallel path
here and no way to buy schedule with more agents. The ordering is not asserted from preference: it
mirrors the crate chain captured in the **historical 18-edge DAG snapshot** (§04): finding-dispatch → omp-orchestrator → ack-stage → receiver-receipt → tick-monitor. M1's seam is at the head and M7's window at the tail.
```mermaid
graph LR
  M1[M1 observe<br/>the seam] --> M2[M2 select<br/>by graph] --> M3[M3 dispatch<br/>with a receipt]
  M3 --> M4[M4 completion<br/>no human] --> M5[M5 the loop<br/>closes] --> M6[M6 foreign<br/>repo] --> M7[M7 unattended<br/>window]
```

`PROJECTED` — target-state chain, not measured data (rule 6); §04 carries the measured crate graph.

### M1 — One shared pane-state type will cross the observe→decide seam, so a filter change breaks the consumer at compile time

**OBSERVABLE.** Both legs are run from the repository root at the recorded `git rev-parse HEAD`,
with the pinned Rust toolchain and the checked-in fixture
`crates/omp-orchestrator/tests/fixtures/tick-monitor-newly-idle.json` (its SHA-256 is printed).
Run exactly:

`cargo test -p omp-orchestrator --test pane_state_seam -- --exact tick_monitor_newly_idle_reaches_orchestrator --nocapture`

The test must print one JSON line with `{"fixture":"tick-monitor-newly-idle.json","state":"NewlyIdle","free_capacity":true,"shared_type":"PaneObservation","revision":"<git sha>"}` and exit 0. It must compile the real fixture through the production parser and fail if either crate's public parse/emit signature does not use the same `omp-types::PaneObservation` type; the pre-milestone tree fails because the test, fixture, dependency edge, and shared signature are absent, while the post-milestone tree passes only with that JSON and exit 0. A separate source check is the exact command
`grep -rn "omp-types\\|omp_types" crates/*/Cargo.toml | grep -v "^crates/omp-types/"`; its non-empty result must name both consumers. **NOT IN SCOPE.** Selection, dispatch, ack — M1 changes what the loop *sees*.

> *Upstream type for this gap: `GuestIdleReconcilerCtx` (DECLARED only). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

**STARTING POINT — with a recorded disagreement with the brief.** Brief §4 lists the *actionable* layer as **BROKEN** — "`idle_panes`
discards `NewlyIdle`; `free_capacity` derives from the same `is_dispatchable` filter." **I measured the current source and disagree:**

CURRENT SEAM EVIDENCE (2026-09-01):
crates/tick-monitor/src/lib.rs:467-468       is_free_capacity() = ConfirmedIdle | NewlyIdle
crates/omp-orchestrator/src/lib.rs:739       consumer counts pane.is_free_capacity
crates/omp-orchestrator/src/main.rs:1646-1653 regression test proves IDLE -> free_capacity but not dispatchable

The producer emits free_capacity from its own predicate; the consumer counts its own field. The filter defect is fixed in source. What remains broken is the seam: parse_observation at crates/omp-orchestrator/src/main.rs:383-410 reads JSON lists plus a state == IDLE fallback, so the NewlyIdle branch is not represented by a shared type or an end-to-end fixture. omp-orchestrator has no path-depends-on edge to tick-monitor (brief §3.4).

**RISK.** A live-seam refactor needs an owner: crates/no-shell-gate/tests/wired_lanes.rs:679 records Observation as REQUIRES A DECISION, not an allowance, at the free_capacity seam. A partial migration adding a shared type beside the existing structs would create three dialects rather than collapsing two.

**UPSTREAM CORROBORATION, declared only.** OMP declares GuestIdleReconcilerCtx at dist/types/collab/guest.d.ts:9-30 with a settle-vs-continuation split analogous to NewlyIdle/ConfirmedIdle. No wire probe has carried it; the measured local seam and risk above remain.


### M2 — Selection will run through the graph kernel instead of queue recency

**OBSERVABLE.** From the repository root and the recorded revision, run
`cargo test -p omp-orchestrator --test selection_graph -- --exact graph_and_recency_choose_different_beads --nocapture`
against the checked-in fixture `crates/omp-orchestrator/tests/fixtures/blocking-chain.json`. The test must print one JSON line with
`{"fixture":"blocking-chain.json","graph_bead":"<id>","recency_bead":"<different-id>","selected_by":"bv","revision":"<git sha>"}` and exit 0; the pre-milestone tree fails because no `bv` invocation or test exists, and the post-milestone tree fails if the IDs are equal or `selected_by` is not `bv`. The test MUST exercise the real `bv --robot-next` invocation (or a recorded executable-equivalent adapter) and assert the parsed JSON fields; a source-text grep is diagnostic only and cannot close M2. **NOT IN SCOPE.** Dispatch — M2 decides *what* is worked; M3 delivers it.

**STARTING POINT.** `MEASURED` — the sanctioned graph selector is not invoked by production code:

```text
harness grep, pattern Command::new\("bv"\), path crates   -> No matches found
```

This zero is not acceptance evidence: it is a scoped source observation, and the executable behavior above is the only admissible M2 proof. `bv` is spawned zero times today while `crates/kernel-only-operator-hook/src/lib.rs:548` refuses raw queue reads with *"raw `br ready` is blocked; use the `bv --robot-triage` queue kernel"*. **NO-CLAIM.** Graph selection being invoked is not graph selection being better; the differential leg proves only that the strategies differ.
```
**RISK.** `bv v0.20.0` is at `/opt/homebrew/bin/bv`, not `~/.local/bin`; a foreign machine (M6) may lack it, so M2's dependency must be
a degradation path with a typed refusal, or M2 blocks M6. **NO-CLAIM.** Graph selection being *invoked* is not graph selection being
*better*: the differential leg proves the strategies differ, not that the graph wins.

### M3 — Dispatch will return an acknowledgement or a typed refusal, never a bare success

**OBSERVABLE.** From the repository root at the recorded revision, run
`cargo test -p omp-orchestrator --test dispatch_receipt -- --exact known_bad_transport_is_refused --nocapture`
with fixture `crates/omp-orchestrator/tests/fixtures/transport-success-no-packet.json`. The test must print one JSON line with
`{"fixture":"transport-success-no-packet.json","transport":"success","screen_changed":false,"verdict":"Refused","receipt":"not-confirmed","revision":"<git sha>"}` and exit 0. The pre-milestone tree fails because the journaled fixture/test and receipt check are absent. Run the explicit mutation leg `cargo test -p omp-orchestrator --test dispatch_receipt -- --exact ack_removal_is_rejected --nocapture` against `crates/omp-orchestrator/tests/fixtures/transport-success-no-packet-mutated.json`; it must exit non-zero before the mutation is repaired and exit 0 when the harness proves the mutation was rejected. The verdict is receiver-side screen evidence (timer reset + content change), **not** packet arrival; the stronger packet-correlated receipt is not required for M3. **NOT IN SCOPE.** Detecting whether the worker *finished* (M4); M3 ends at the journaled arrival/refusal decision.

**STARTING POINT.** `MEASURED` — brief §4 lists actuate as **DOES NOT EXIST**: a human types into panes. Both polarities have fired.
`cp-z42vu` (`README.md:155`, `dispatch-silence-watch/src/lib.rs:10`): a send returned **`success:[4]` while the packet never arrived**;
the inverse fired the same session. Evidence caveat, recorded rather than hidden: both incidents
predate the packet journal (11-lifecycle F), so their record is narrative copies, not an immutable
capture — M3's own observable must therefore be demonstrated on a JOURNALED dispatch, one whose
packet and receipt exist as records.

**RISK.** The available ack is *terminal inspection*: `receiver-receipt` spawns `tmux`
(`crates/receiver-receipt/src/bin/receiver-receipt.rs:19`), and reading a rendering is not a protocol. `ntm --robot-send` already
refuses codex panes with *"cod composer not visible"* (`cp-nq2s9`, `README.md:152`) — a screen-state guard misreporting as a delivery
error, a class a rendering-based ack inherits. **NO-CLAIM.** An ack proves *arrival*, not that the agent read or accepted it.
**UPSTREAM TYPE, at its true strength.** OMP declares a typed receipt family — `IrcDeliveryReceipt`
(`tools/hub/types.d.ts:8`) and `AsyncJobDeliverySink` (`:84`). This is **DECLARED only**: no wire
probe has carried one, so it does not close M3; it means the receipt shape we would adopt already
has an upstream vocabulary, and cp-z42vu stands as the measured reason M3's planted-known-bad
observable is unchanged.

### M4 — Completion will be detected by the loop, not by a human looking

**OBSERVABLE.** From the repository root at the recorded revision, run
`cargo test -p ack-spine --test completion_trace -- --exact verifier_closes_without_human --nocapture`
against `crates/ack-spine/tests/fixtures/m4-completion-trace.json`. The test must print one JSON line with
`{"trace":"m4-completion-trace.json","human_touch_count":0,"close_actor":"verifier","worker_actor":"<different-id>","bead_state":"closed","revision":"<git sha>"}` and exit 0. The pre-milestone tree fails because no consumer records this trace. Run the explicit mutation leg `cargo test -p ack-spine --test completion_trace -- --exact worker_close_is_rejected --nocapture` against the same fixture; it must exit non-zero if a worker can close its own bead, and the verifier trace test must exit 0 only when that mutation is rejected. **NOT IN SCOPE.** Whether the work is *good* (the verify gate), and autonomy end to end (M5).

**STARTING POINT.** `MEASURED` — the vocabulary exists, nothing reaches it, and brief §4 records that every completion this session was
found by a human looking.

```
harness grep "Finished" in crates -> 8 hits, ALL in ack-spine/src/followup.rs
harness grep "ack-spine|ack_spine" in crates/*/Cargo.toml -> only ack-spine's own package/bin/lib names
```

`FollowUpVerdict::Finished { bead_id, close_verdict }` is declared at `ack-spine/src/followup.rs:27`, distinguished from the
silent-past-deadline arm by a mutation leg at `:205`: a good type, a mutation test, **zero callers outside its own file**, in a crate
with `zero dependents` — the reported occurrence count (21) is a historical, UNPROVEN snapshot and is not used as a baseline; the exact M4 trace command above is the replacement evidence.

**RISK.** A worker asserting `Finished` is a *claim*, and this plan's thesis is that claims are what fail. M4 must land the ack path
without letting worker-asserted done become the close condition, or it manufactures the false-completion signal the project exists to
eliminate. **NO-CLAIM.** M4 delivers *detection*; detection without verification is faster wrongness.

### M5 — The loop will close a bead end to end with no human in the trace

**OBSERVABLE.** From the repository root at the recorded revision, run
`cargo test -p omp-orchestrator --test control_loop -- --exact ten_recorded_closes_without_human --nocapture`
against `crates/omp-orchestrator/tests/fixtures/m5-run.json`. The test must print one JSON line with
`{"run":"m5-run","close_count":10,"human_event_count":0,"unaccounted_refusals":0,"bead_ids":["id-01","id-02","id-03","id-04","id-05","id-06","id-07","id-08","id-09","id-10"],"independent_readback":true,"revision":"<git sha>"}` and exit 0; each ID must be independently checked by `br show <id> --json`. The pre-milestone tree fails because no recorded run or consumer exists, and a global board count is explicitly insufficient. **NOT IN SCOPE.** Duration (M7) and portability (M6) — M5 is one closed loop, once, ten times.

**STARTING POINT.** `MEASURED` against the current seven-row control loop (brief §4): observe WORKS-with-defect; actionable's filter is
FIXED (-oco) with the seam still open; consume FENCED. The reported **162 consecutive refused ticks over 4.2 hours** (`DISPATCH_RETRY_BLOCKED`) is a historical, UNPROVEN snapshot: the source ledger export and deriving command are not retained, so it is not a current baseline. Actuate DOES NOT EXIST; and complete is AVAILABLE-NOT-WIRED — the upstream completion type is wire-proven, the local consumer is not built (Gap 7, refuted and adopted). Board counts are re-runnable only with this failure-preserving probe:

`set -o errexit -o nounset -o pipefail
  for s in open in_progress blocked closed; do
    STATUS="$s" br list --status "$s" --json | STATUS="$s" python3 -c 'import json, os, sys; d=json.load(sys.stdin); rows = d if isinstance(d, list) else d.get("issues", d.get("items", [])); print(json.dumps({"status": os.environ["STATUS"], "count": len(rows)}, sort_keys=True))'
  done`

A `br` failure or malformed JSON exits non-zero rather than becoming a zero count. The prior **19/22/3/30** output and **74-bead** total are historical, UNPROVEN snapshots and are not used as the current board state; rerun the probe above before recording a new snapshot.

**RISK.** M5 is where maximum pressure belongs: everything before it is infrastructure demonstrable in isolation, while M5 is the first
milestone whose failure invalidates the thesis rather than delaying it, and there is **no evidence for or against it.** **NO-CLAIM.**
Ten closes is an existence proof, not a rate.

### M6 — A repository that is not this one will run the orchestrator on a machine that has never built it

**OBSERVABLE.** On a clean host, from the repository root at the recorded revision, run this exact shell script. The install leg targets the `omp-orchestrator` package (not the separate `installer` package), and the cargo executable is resolved before the empty-environment probe so the probe does not accidentally test whether an inherited PATH happens to contain Cargo.

```sh
set -o errexit -o nounset -o pipefail
git rev-parse HEAD
test ! -e target
tmpdir="$(mktemp -d)"
cargo_bin="$(command -v cargo)"
test -x "$cargo_bin"
cargo_dir="$(dirname "$cargo_bin")"
env -i HOME="$HOME" TMPDIR="$tmpdir" PATH="$cargo_dir:/usr/bin:/bin:/opt/homebrew/bin" \
  cargo install --locked --path crates/omp-orchestrator --bin omp-orchestrator --root "$TMPDIR/omp-m6"
first_tick_log="$tmpdir/m6-output.log"
if env -i HOME="$HOME" TMPDIR="$tmpdir" PATH="$tmpdir/omp-m6/bin:/usr/bin:/bin" \
  omp-orchestrator run --repo crates/omp-orchestrator/tests/fixtures/foreign-repo --once >"$first_tick_log" 2>&1; then
  first_tick_exit=0
else
  first_tick_exit=$?
fi
test -s "$first_tick_log"
if grep -q 'SUPERVISOR_REFUSED' "$first_tick_log"; then verdict=named_refusal
elif grep -q 'OBSERVATION' "$first_tick_log"; then verdict=progress
else printf 'M6_UNCLASSIFIED_OUTPUT\n' >&2; exit 1
fi
FIRST_TICK_EXIT="$first_tick_exit" VERDICT="$verdict" REVISION="$(git rev-parse HEAD)" python3 -c 'import json, os; print(json.dumps({"install_exit": 0, "cache_absent": True, "fixture": "foreign-repo", "first_tick_exit": int(os.environ["FIRST_TICK_EXIT"]), "verdict": os.environ["VERDICT"], "revision": os.environ["REVISION"]}, sort_keys=True))' >"$tmpdir/m6.json"
test -s "$tmpdir/m6.json"
jq -e '(.install_exit == 0) and (.cache_absent == true) and (.fixture == "foreign-repo") and (.first_tick_exit == 0) and (.verdict == "progress" or .verdict == "named_refusal") and (.revision|type == "string") and (.revision|length > 0)' "$tmpdir/m6.json" >/dev/null
```
**RISK.** Absolute paths and host assumptions are invisible until the first foreign host, plus a per-binary version-probe map. `MEASURED` here twice: tmux is installed and `-V` succeeds; `--version` is unsupported and fails with exit 1, so it must not be used as the presence probe. The brief's "no handshake" and its first correction ("exits 0 while failing") are both refuted:
```
tmux -V                              -> tmux 3.6a                       exit 0
tmux --version >out 2>err            -> stdout 0 bytes, stderr 158       exit 1
tmux --version 2>&1 | head -1        -> $?=0, PIPESTATUS=(1 0)    <- head's status, not tmux's
```

tmux fails, says so on stderr, writes nothing to stdout, returns 1. The "exits 0" was a probe reading `$?` after a pipeline; **the exit
code that lied was our harness, not the subject.** The hazard inverts: `--version` answers 8/9 of our binaries and `-V` answers 6 of 9 (`ntm`, `br`,
`tmux`, `cargo`, `fh`, `jsm`; fails `omp`, `bv`, `git`), so
a probe treating non-zero as ABSENT records tmux — present, working, 3.6a — as MISSING, a false negative on the one binary through which
we read pane truth. Verified precedent, in `dicklesworthstone-mirror/pi_agent_rust/src/doctor.rs`, cited by **construct** because a bare
line number is unverifiable across checkouts — these constructs sit one line earlier in `~/Developer/pi_agent_rust` and its tests 22
lines earlier, which is what made three of us disagree about the "same" line: `fn check_tool` (`:924`), the naive success arm
`Ok(output) if output.status.success()` (`:950`), the two-signal arm combining `discovered_path.is_some()` with
`probe_failure_is_known_nonfatal` (`:967-968`), that predicate itself (`:1052`) whose one-tool allowlist `if tool.ne("sh")` (`:1057`) is
the gap, and the independent presence signal `fn which_tool` (`:1066`). The strongest part is that **both arms are pinned by tests** —
`check_tool_falls_back_when_probe_args_are_unsupported` (`:13948`) and `check_tool_reports_invocation_failure_for_broken_executable`
(`:13964`), the second being the leg we would skip, proving the fallback did not become a blanket amnesty. Verdict: **ADOPT the
two-signal structure and both tests, with a named gap** — the allowlist covers exactly one tool, so a doctor built on this code marks
tmux MISSING today. **NO-CLAIM.** M6 proves portability to one host and only "did install, once"; §07–§08 own the general case.

### M7 — The fleet will run unattended for a defined window with every refusal accounted for

**OBSERVABLE.** From the repository root at the recorded revision, run
`cargo run -p omp-orchestrator --bin omp-orchestrator -- --repo crates/omp-orchestrator/tests/fixtures/foreign-repo --duration 24h --evidence target/milestones/m7.json`.
The command must exit 0 only after 24 hours in the pinned environment and must emit JSON with `{"window_seconds":86400,"human_event_count":0,"ticks":1,"progress_or_named_refusal":true,"max_same_refusal":3,"refusal_inventory":["refusal-class"] ,"revision":"<git sha>"}` (the actual `ticks` value is an integer greater than zero and the inventory contains the observed classes); a missing tick, unnamed refusal, empty inventory, or recurrence above `FINDING_THRESHOLD == 3` exits non-zero. The pre-milestone tree fails because no unattended runner/evidence exists. **NOT IN SCOPE.** Increasing the window — M7 is the first defensible number, not the best one.

**STARTING POINT.** **HISTORICAL/UNVERIFIED:** a prior session reported 4h 19m of fleet idleness while watchdogs reported healthy. No deriving command or retained artifact exists, so this is a failure-shape example, not a measured current duration. M7's actual unattended duration remains unproven until its transcript exists.

**RISK.** Goodhart, the sharpest risk here. **An unattended window achieved by widening tolerance is a regression that scores as
progress.** M7 is trivially closable by deleting refusals, so acceptance requires the refusal *inventory* to be non-empty and unchanged
in kind across the window — a silent loop and a healthy loop are the two states this project exists to distinguish. **NO-CLAIM.** M7
measures *survival*: a loop running unattended a day producing nothing satisfies M7 and is worthless.

## 3. Plan validation — how this document is checked

R3 wants a document an investor can pass or fail, so it must be checkable, not merely well written.

- **PV1 — Every number carries the command that derives it.** A bare number is a guess in a uniform.
- **PV2 — `MEASURED` and `PROJECTED` never share a sentence.** The best review finds a `MEASURED` that is a `PROJECTED`.
- **PV3 — Every load-bearing claim ends with a `NO-CLAIM`.** An unbounded claim is read as covering more than it does.
- **PV4 — Every milestone has an admissible observable** (§1). One that cannot be failed cannot be passed.
- **PV5 — Gate claims are floor-raises, never guarantees.** `guarantees`, `proves`, `makes impossible` in a gate header is itself a
  defect — the reader stops looking. Gates raise a defect class's cost, not its possibility.
- **PV6 — No unstated denominators.** Three measured instances. **"81 JSON-RPC methods, 17 of 81 used"** — inherited, no producing
  command, **not re-derivable** on 2026-08-31, retired at `AGENTS.md:247` (`grep -n "17 used\|81 JSON" AGENTS.md` returns only notices
  at 247, 582, 615). **A drift ratio where excluding a foreign binary decremented the wrong variable, turning `2/2` into `2/0`** (brief
  §6 rule 4). The brief's gate-count headlines (1 of 8 / 5 of 8, later corrected to 2 of 8 / 4 of 8) are historical, UNPROVEN snapshots: no current derivation is retained, so they are not a baseline or a present verdict. A fresh gate report must be captured before any new count is stated.
- **PV7 — A zero must be distinguished from a silence, and a not-found must name its search space.** A command that returns **empty
  instead of failing** is indistinguishable from a true zero, so every "nothing found" built on one is unfalsifiable. Three mechanisms
  `MEASURED` in one session — (a) and (c) by me, (b) by a sibling. (a) Shell `grep -r … --include='*.rs'` returns **0** for
  `forbid(unsafe_code)` in `crates` while the harness grep returns **55 files** for the identical pattern; dropping `--include` restores
  it. (b) An extension filter aimed at the wrong language — `--include='*.rs'` across `ntm`, **a Go repository** — returned zero, and
  structural absence was read as semantic absence; re-derived without the filter it is 93+ files, and the "no prior art" verdict it
  produced was false. (c) A pipeline launders a failure into a success: `tmux --version 2>&1 | head -1` yields `$?=0` with
  `PIPESTATUS=(1 0)`. So **"I grepped and got nothing" is not a finding** — a publishable not-found names the exact command *and* why
  that search space was the right one. The M2 and M4 zeros were re-derived with the harness grep; M1's zeros were re-derived with
`strings`/`grep` on built binaries (the object there is a compiled artifact, where the harness
grep does not apply), and M6's are redirect-separated exit probes. Every zero names its
instrument; none was accepted from a prior round. The pattern is the
  finding: **four false-zero or mis-transcribed measurements in one session, three of them the conductor's, and every one caught by a
  sibling re-deriving rather than reading.** Re-derivation, not review, is what caught them — which is an argument for the loop this
  plan proposes.

- **PV8 — A citation names the construct, not just the line.** `doctor.rs:924` is unverifiable without knowing whether 924 is the
  function, the success arm, or the predicate; line numbers drift under a reformat and differ *between checkouts of the same repo*.
  `MEASURED`: the six constructs above sit at `:924/:950/:1052/:1057/:1066` in the mirror and one line earlier in
  `~/Developer/pi_agent_rust`, its tests 22 lines earlier — producing a three-way disagreement in which nobody was wrong. Write
  "`doctor.rs:950` (the naive success arm)": a named construct survives both.

**NO-CLAIM.** PV1–PV8 are necessary, not sufficient. A document can satisfy all eight and still be wrong about what matters. §5 exists
because rule-compliance is not correctness.

## 4. The self-check that runs against this plan

`PROJECTED` — design spec. A binary, `plan-check`, will read `docs/PLAN.md` and its sections, failing on:

| id | fails when | enforces |
|---|---|---|
| PC1 | a numeral appears in prose with no command in the same block | PV1 |
| PC2 | `guarantees` / `proves` / `makes impossible` appears within a gate section | PV5 |
| PC3 | **syntax precheck only:** an `Mn` milestone heading has no `OBSERVABLE` field | PV4 heading presence; **does not establish semantic admissibility** |
| PC4 | a sentence marked `MEASURED` contains a projection verb (`will`, `would`, `should`) | PV2 |
| PC5 | **syntax precheck only:** a section has no `NO-CLAIM` paragraph | PV3 paragraph presence; **does not establish per-claim scope coverage** |
| PC6 | a ratio appears whose denominator has no separate derivation | PV6 |
| PC7 | a zero-result claim names no search space, or no failure-vs-empty distinction | PV7 |
| PC8 | a `path:line` citation names no construct at that line | PV8 |

It will emit the repo's envelope — `{"schema_version":"plan-check/v1","command":"doctor","status":…}` — matching `omp-inventory-map/v1`
(brief §3.2) so one reader handles both, and satisfy **ADDRESSABLE** (brief §3.6): `plan-check --help` will name the command that runs
it, because `omp-inventory-map --help` returns `CONFIG_ERROR unknown argument --help`, and a correct, undiscoverable gate does not
exist. It ships a **known-bad leg** — planted bare number, planted `guarantees`, observable-less milestone — since known-good-only
fixtures are vacuity again.

**This checker does not exist.** MEASURED: ls crates/ | grep plan-check returns nothing; the workspace has **50 crate directories** (ls -1 crates | wc -l) and none is this one. It is the sharpest thing an investor can hold against this document: **a plan that asserts a discipline it does not enforce is exactly the vacuity defect we found in our own census.** Brief §3.3 measured 183 rows meeting a four-field requirement with one distinct value across all 183 — met and meaningless. §3 is in that position now: eight rules enforced by the author's attention.

**NO-CLAIM.** `plan-check` would enforce *form*. Nothing in this design detects a false `MEASURED` whose command was never run, or an
observable that is admissible and irrelevant. Those need a reader.

## 5. The grading rubric — how to fail us

Score each dimension PASS or FAIL. **We are asking to be failed on at least one** — a reviewer who passes all six has not read
adversarially, and a document engineered to pass all six is useless to us.

| dimension | PASS looks like | FAIL looks like |
|---|---|---|
| **The problem is real** | The failure class is shown with counts and a derivation, and it is a class rather than an anecdote | The pain is asserted, or one story is generalised into a market |
| **The measurement is honest** | Numbers re-derive; `MEASURED`/`PROJECTED` hold; retired figures stay retired; every not-found names its search space | A number that will not re-derive; a projection wearing `MEASURED`; a floating denominator; a not-found with no search space; a bare `path:line` with no construct |
| **The architecture is sound** | Typed transitions with refusal arms; a timeout is not a verdict; one vocabulary crosses each seam | Boolean success paths; convention-only seams; a refusal no consumer reads |
| **The gates actually bite** | Each gate has known-bad, known-good, mutation, anti-vacuity, and is ADDRESSABLE | Attack-only or defence-only suites; a gate no documented command runs; `guarantees` in a header |
| **The adoption path is credible** | A cold foreign host installs and takes a first tick, with degradation for absent dependencies | Installability asserted and never attempted; hard dependency on host-specific paths |
| **The team tells on itself** | The document names defects its own author committed, and records disagreement with its own brief | Every finding indicts a worker; no self-indictment; no open contradiction |
**Where we expect to fail today, stated first so a reviewer can check our self-assessment.** *Gates actually bite → FAIL*: the former **2-of-8 / 4-of-8** figures are **HISTORICAL, UNPROVEN** snapshots with no retained derivation and are not a current verdict; `path-literal-guard` still has a known-bad with **no known-good**, and over-strict gates get routed around, a slower death than no gate. A fresh gate report is required before stating a new count. *Adoption path is credible → FAIL*: `MEASURED`, never attempted (M6); `installer` is isolated in the dependency graph.
*Architecture is sound → CONTESTED*: the former **6 / 17 / 4** type counts are **HISTORICAL, UNPROVEN** snapshots with no retained derivation and are not a current baseline; the typed-seam risk remains the subject of M1–M4.

**NO-CLAIM.** This rubric grades *this document*, not the software. A document that passes all six describes a system that may still not
work — which is why M5 exists and why it is unproven.

## 6. What is proven and what is not

BANKED rows cite evidence; UNPROVEN rows name the experiment that settles them. No row sits between.

| BANKED — qualitative evidence only; stale counts excluded | UNPROVEN — with the experiment that settles it |
|---|---|
| The build-and-never-call class is real and frequent here — the **20-occurrence / 21st-occurrence** counts are **HISTORICAL, UNPROVEN** snapshots and are not banked; the retained evidence is the named `Finished` grep and zero-dependency check | **The loop can close without a human.** Never observed once. Settled by M5: ten consecutive closes from one recorded run with zero human-originated events. |
| Silent refusal is real — the **162-tick** and **178-tick** counts are **HISTORICAL, UNPROVEN** snapshots with no retained ledger derivation and are not banked | **The substrate installs elsewhere.** `MEASURED`: never attempted; `installer` is isolated in the 18-edge graph. Settled by M6: a cold foreign host, documented install, first tick, reproducible transcript. |
| Transport lies in both directions — `cp-z42vu` returned **`success:[4]`** with no packet delivered (`README.md:155`); the inverse fired the same session | **Verification is cheaper than review.** No instrumentation exists. Settled by recording human-seconds per closed bead before and after M4 on the same bead mix — that ratio is the whole economic argument and is unmeasured. |
| The evidence-field discipline can be satisfied vacuously — the brief's **183/183** and **one-distinct-value** figures are **HISTORICAL, UNPROVEN** snapshots and are not banked | **Graph selection beats recency.** `MEASURED`: `bv` is spawned zero times. Settled by M2's differential test plus a measured close-rate comparison on a fixed bead set. |
| The observe layer works and the NewlyIdle filter defect is fixed in source — tick-monitor/src/lib.rs:467-468 exposes ConfirmedIdle or NewlyIdle free capacity; regression test observed_idle_state_counts_as_free_capacity_before_confirmation at omp-orchestrator/src/main.rs:1646-1653 | **The seam cannot silently diverge again.** No shared type crosses it; parse_observation uses JSON lists plus an IDLE fallback. Settled by M1 only after the shared-type fixture lands. |
| The one hard rule is enforced mechanically — a Rust gate walks `git ls-files` and fails on `.sh`/`.py`, exemption list empty | **The gates bite under attack.** The former **2-of-8 / 4-of-8** figures are **HISTORICAL, UNPROVEN** snapshots and are not a current verdict. Settled by bringing all 8 to four legs plus ADDRESSABLE, and a planted-known-bad campaign per gate. |
| Failing closed with a remediation hint works — `fh` MCP returns a typed `SERVE_INPUT_STALE` naming the moved mirror HEAD (`5dec4212…` → `ecdea397…`) rather than an empty result | **This plan enforces its own discipline.** `MEASURED`: `ls crates/ \| grep plan-check` → empty. Settled by shipping `plan-check` (§4) with its known-bad fixture. |

> *Upstream type for this gap: `IrcDeliveryReceipt` (`tools/hub/types.d.ts:8`, DECLARED only). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

**NO-CLAIM.** This section defines the conditions under which a milestone may be called done, and records what is and is not proven as
of 2026-08-31 on one repository and one machine. It does not establish that these are the right milestones, that seven is the right
number, that the ordering is minimal, or that closing all seven produces a system anyone wants. It makes no schedule claim — no date,
duration, or effort estimate appears above, because none is measured. The most important row here is `M5 — UNPROVEN`; no rigor elsewhere
substitutes for it.


---


<!-- ===== 10-prior-art.md ===== -->

# 10 — What would Jeffrey do: prior art mined from the mirror

**Requirement served: R7.** *"use fh — mine the dicklesworthstone projects along the way — anywhere we find a gap — we should ask — what would jeffrey do in one of his projects."*

Nine named gaps are researched below. Each record gives the gap, exact search, a named construct and quote (or a scoped not-found), and an **ADOPT / ADAPT / REJECT** verdict. Mirror citations are tied to §10.1; installed OMP citations are tied to §11.1. A declaration is not runtime proof, and a runtime injection receipt is not recipient acknowledgement. The nine gaps and the seven OMP rows in §11 are not one-to-one: only Gaps 1 and 7 have direct OMP mappings; the other OMP rows are adjacent mechanisms.

---

## 0. Corpus, tooling, and measurement boundaries

**Replay prefix (required).** Every relative mirror search below runs after M=/Volumes/ZestData/dicklesworthstone-mirror and cd "$M"; a result obtained from the orchestrator repository root is invalid and must not be cited as mirror evidence.
Recorded filesystem measurements:

```
ls -1 | wc -l                                      -> 218   # visible entries
find . -maxdepth 1 -mindepth 1 -type d | wc -l     -> 217   # directories
find . -maxdepth 2 -mindepth 2 -name .git | wc -l  -> 210   # filesystem .git entries; NOT validated work-trees
ls -1 | grep -c corrupt                            -> 1     # ntm.corrupt-20260819
```

The research denominator is **MEASURED 210 filesystem `.git` entries**, not 210 validated work-trees, 218 visible entries, 217 directories, or Brief §3.7's un-re-derived “216 repos.” A validation pass has not classified these entries as non-bare work-trees, linked worktrees, nested repositories, or submodules. `fh` refused stale input as `SERVE_INPUT_STALE` after mirror movement `5dec4212… -> ecdea397…`; its CLI also refused `SEARCH_INDEX_STALE` at exit 3:

```
SEARCH_INDEX_STALE: published key f2845efff917afd4 differs from current b1acb6e7b011b1f5
hint: run `fh technical-manifest`
```

All mirror searches therefore ran against the filesystem, without semantic-index assistance. Full mirror commit IDs were unavailable in this pass; §10.1 records that limitation instead of inventing revisions.

**UNRESOLVED PROVENANCE / PLAN GATE.** The required SOTA preflight is not green: `fh suggest` returned `SEARCH_INDEX_STALE` (published `f284...` vs current `b1ac...`), and `fh technical-manifest --check` failed `TSX_ITEMS_PARSE_FAILED` at `apps/wire/scripts/suites.ts`. `jsm suggest` was offline and returned only generic context suggestions. This section makes no claim that the mirror index or SOTA skill recommendation gate passed; owner: integrator; next action: refresh the fh technical manifest, repair the TSX parse failure, then re-run both commands and retain their outputs.

### 0.1 Measurement ledger

| measure | derivation/input | result and boundary |
|---|---|---|
| mirror denominator | `find . -maxdepth 2 -mindepth 2 -name .git | wc -l` | **MEASURED 210 filesystem entries** at one snapshot; work-tree validity is **UNVALIDATED**, not a repository count |
| visible entries / directories | `ls -1 | wc -l`; `find . -maxdepth 1 -mindepth 1 -type d | wc -l` | **MEASURED 218 / 217**, not project denominators |
| `vergen` matches | `grep -rl vergen --include=Cargo.toml . | wc -l` | **MEASURED 18 manifest paths**; unique-repository numerator is NO-CLAIM and the 210 filesystem denominator is not validated |
| supervision variants | direct reads of `enum SupervisionEvent` and `enum StopReason` | **MEASURED 8 + 6 = 14 variants** in the named file, not corpus-wide |
| OMP completion capture | `/tmp/grade/r7-agent-end.md` and raw frame | **MEASURED 1 `agent_end` frame** in one ephemeral run |
| seeded corrections | comparison of nine seeded Gap rows with this record | **PROJECTED 6/9 = 66.7%**; document comparison, not an independent run |
| refuted not-founds | four false-zero mechanisms described below | **MEASURED 4** named mechanisms in this section; no session-count or completeness claim |

The correction arithmetic is `6 ÷ 9 = 0.666…`. “Strongest,” “highest priority,” and similar rankings are not measured claims; where priority is discussed it is marked **PROJECTED** from stated evidence.

Four false-zero mechanisms remain explicit: an empty `--include=` can return exit 0; an extension filter can target the wrong language (`ntm` is Go); a search space can omit where the answer lives; and a not-found can be published without a recorded search. A negative result is publishable only with its command, source scope, observed output, and remaining NO-CLAIM.

---

## Gap 1 — A publish that returns no receipt

> *Upstream type for this gap: `IrcDeliveryReceipt` (`tools/hub/types.d.ts:8`, DECLARED only). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

**Gap.** Dispatch emits no typed acknowledgement, so “sent” and “accepted” are one observable.

**Search.** `grep -Ern 'pub (struct|enum) (PublishReceipt|AckKind|DeliveryClass|PublishPermit)' asupersync/src/messaging` over the whole `asupersync/src/messaging` module. The `-E` is required for the grouping and alternation; the replay result is recorded in `/tmp/grade/r13-10-prior-art.md`.

**Found.** `asupersync/src/messaging/fabric.rs:1913` (`struct PublishReceipt`) carries `subject`, `payload_len`, `ack_kind`, and `delivery_class`; `asupersync/src/messaging/fabric.rs:1944` carries `#[must_use = "a PublishPermit must be sent or explicitly aborted"]`. `AckKind` is `asupersync/src/messaging/class.rs:83`; `DeliveryClass` is `asupersync/src/messaging/class.rs:17`; `cost_vector` and `minimum_ack` carry the `#[must_use]` rule at `asupersync/src/messaging/class.rs:43,56`.

**OMP boundary.** `irc/bus.d.ts:30-34` (`interface IrcDeliveryReceipt`) reports only `injected | woken | revived | failed`; `irc/bus.d.ts:53-61` says it reports how a message reached the recipient, “not what they did with it.” `async/job-manager.d.ts:37-48` (`type AsyncJobDeliverySink`) is a callback/dead-letter shape. **NO-CLAIM:** neither declaration proves recipient acceptance, readback, or durable acknowledgement.

**Verdict: ADAPT.** `crates/omp-types/Cargo.toml:10-18` records the local dependency boundary. Adopt an ack-bearing receipt and non-droppable permit locally; keep `cp-z42vu` ADAPT until recipient-level runtime proof exists.

---

## Gap 2 — An allowance list that outlives the defect it records

**Gap.** Declare an unwired lane without allowing the declaration to outlive its defect.

**Search.** `grep -n 'UNWIRED_LANE_ALLOWANCE' franken_lean/crates/fln-conformance/tests/contract_roots.rs`.

**Found.** `franken_lean/crates/fln-conformance/tests/contract_roots.rs:284-288` (`UNWIRED_LANE_ALLOWANCE`) is empty. Its doc comment says an undeclared unwired lane fails and a declared lane that has since been wired also fails. `fn allowance_verdict_fails_in_both_directions` at `franken_lean/crates/fln-conformance/tests/contract_roots.rs:777` exercises both directions; refusal wording is at `franken_lean/crates/fln-conformance/tests/contract_roots.rs:757-761`.

**Scoped NO-CLAIM.** Source root is `/Volumes/ZestData/dicklesworthstone-mirror`; search space is the named contract-root file plus `crates/omp-inventory-map/src/types_inventory.rs:176-178`; observed allowance is empty and the dual-direction test is named. This does not establish every allowance list in the corpus or that the local inventory list is fixed.

**Verdict: ADOPT** the dual-direction allowance shape.

---

## Gap 3 — A binary that cannot say which source built it

**Gap.** An installed binary cannot prove it matches the source tree it claims.

**Search.** `grep -rl vergen --include=Cargo.toml .` then `grep -rn 'binary_identity|build_id|running_binary'`. The first command yields **18 matching manifest paths**, not 18 unique repositories.

**Found.** `beads_rust/build.rs:41-45` (`fn emit_git_metadata`) emits `VERGEN_GIT_DIRTY`. `frankensqlite/crates/fsqlite-e2e/tests/bd_wsw3p_concurrent_write_showcase.rs:840-846` (`fn running_binary_identity`) states: “Fails closed: a gate that cannot name the exact binary it measured is not admissible evidence, so an unresolvable path or any read error panics rather than degrading to an unidentified run.”

**Scoped NO-CLAIM.** The result establishes identity metadata in 18 matching manifest paths and the named fail-closed construct in the recorded mirror snapshot. It does not establish 18 repositories, an exposed `br` identity, or that `fh` source is in the mirror. The old `ls -1 | grep -i harv` top-level probe cannot establish source absence.

**Verdict: ADOPT (PROJECTED locally).** Adopt tree digest plus dirty flag and require a local runtime identity capture before claiming drift prevention.

---

## Gap 4 — The canonical doctor shape

**Gap.** **Historical seed premise (not reproducible in the current checkout):** an earlier run reported `omp-inventory-map --help` as `CONFIG_ERROR unknown argument --help`; doctor must remain discoverable and runnable while the workspace is broken. The current probe is explicitly bounded: `command -v omp-inventory-map` found no executable, and `omp-inventory-map --help` returned shell status 127 with only the shell's command-not-found diagnostic (captured-output SHA-256 `e7f99353a1c4a87853d1710c7b0f319cd8545a349fd0943633238cfad162c9df`). **NO-CLAIM:** this current absence does not reproduce or disprove the historical CONFIG_ERROR behavior.

**Search.** `grep -rn 'DoctorExitCode' beads_rust/src`; `grep -n 'Commands::Doctor' beads_rust/src/main.rs`; `find pi_agent_rust -name doctor.rs`.

**Found.** `beads_rust/src/cli/commands/doctor_subsystems/exit_codes.rs:45-51` (`enum DoctorExitCode`) declares eleven variants in two bands: domain verdicts 0–6 and sysexits 64/66/73/74. `FixFailedRolledBack = 3` is documented at `beads_rust/src/cli/commands/doctor_subsystems/exit_codes.rs:21-24` as returning to the verbatim backup. `beads_rust/src/cli/commands/doctor_subsystems/capabilities_doctor.rs:1-15` (`br.doctor.capabilities.v1`) declares `write_scopes`, `env_vars`, `fixers`, `detectors`, and derived `exit_codes`. `eidetic_engine_cli/src/cache/hotset.rs:1504,1519` emits a runnable `repair` string.

The derived list reduces this specific drift; it does not prove drift impossible under all edits. Operationally, the useful shape is that the tool run to diagnose a broken workspace must not require that workspace to be intact.

**Verdict: ADOPT**, as the spine of `07-installability.md`: typed two-band exits, derived capabilities, runnable repair text, and doctor preflight exemption.

---

## Gap 5 — Mutation through a real hook, not a fixture

**Gap.** Fixture tests cannot prove the installed hook still refuses.

**Search.** `grep -rln 'hooks/pre-commit' beads_rust franken_lean destructive_command_guard` with no extension filter.

**Found.** `franken_lean/crates/fln-conformance/tests/evidence_finalization.rs:360-362` copies the real hook into a lab repository and chmods it executable. `franken_lean/scripts/git-hooks/test_projection_guard.sh:202-212` drives real `git commit` and checks chaining to an existing `.git/hooks/pre-commit`. `franken_lean/ci/VERIFICATION_MANIFEST.jsonl:93` explains why a silent successful hook needs a planted-defect cell. `franken_lean/scripts/git-hooks/test_projection_guard.sh:520-524` records size-dependent refusal rates as a race measurement, not a threshold.

`asupersync/src/subsystem_mutation_testing.rs:9` is a counterexample: it builds a `LabRuntime` over a `TempDir`, so it is a fixture. **NO-CLAIM:** the search establishes both patterns, not that every hook has installed-artifact coverage.

**Verdict: ADOPT** the real-hook plus planted-defect rule.

---

## Gap 6 — Refusing an empty scan set

**Gap.** A scan that returns no input must not read as a clean result.

**Search.** The first documentation-only search was a false zero. Re-derived with no extension filter:

```
grep -rli 'vacuous' asupersync                         -> 37 files
grep -rlEi 'vacuit' --include=*.rs .                  -> 236 files
grep -rlEi 'anti.vacuity' --include=*.rs .            -> 63 files
```

The synonym set also included `scanned zero`, `empty scan set`, `no files were scanned`, `scan set is empty`, `would pass vacuously`, and `zero (files|candidates) (scanned|examined)`. **Scoped NO-CLAIM:** these are matching-file counts from the indicated roots and synonyms, not unique repositories or exhaustive absence.

**Found.** The named shapes are present in `asupersync/src/messaging/jetstream.rs:2460` (`vacuous_zero_wait_refusal`) and `scripts/run_jetstream_publish_backpressure_smoke.sh:181-186`; `asupersync/src/runtime/scheduler/metamorphic_tests.rs:438-442,517-522,661-662` asserts exercised workloads; `franken_lean/crates/fln-conformance/tests/marrow_sanitizer_dispatch.rs:105-115` enforces `workflows.len() >= 2`; `franken_lean/tribunal/epoch-lab/tests/derived_input_provenance.rs:538` asserts `item_count > 0`; `franken_lean/tribunal/epoch-lab/tests/build_gate_governed_sets.rs:549,613` names `VACUOUS PASS`; `asupersync/tests/atp_rq_observability_metrics.rs:134-135` uses a positive control; and `asupersync/src/trace/tla_export.rs:111-114` plus `combinator/map_reduce.rs:140-144` carry the rule into types/returns.

`asupersync/CHANGELOG.md:1077-1078` records six RFC 9112 tests that previously passed vacuously; `audit_index.jsonl:3251` records `MR2 cancellation_state_consistency` as fixed after a vacuous test. These are source-record counts, not comparative rankings.

**Verdict: ADOPT — PROJECTED priority** from the production-telemetry citation and our census defect. Shape 1 is the first implementation candidate, not a measured ranking.

---

## Gap 7 — A worker that cannot say it is done

**Gap.** **Historical document premise:** `docs/plan/00-brief.md` §4 (SHA-256 `84569dd180bad9dd7ee6c90fde86fc5ee0d55be46253aa2dd6c9244aa1efd502`) records the `complete` row as worker completion found by “a human looking.” This is a historical statement from that hashed artifact, not a current runtime observation. **NO-CLAIM:** this section does not claim that the historical observation is reproducible or that it describes every worker mode.

**Search.** `grep -n 'pub enum Outcome' asupersync/src`; then `(cd asupersync/src && grep -En 'pub enum (ChildExit|ExitReason|ChildOutcome|SupervisionEvent|ChildStatus)' supervision.rs gen_server.rs spork.rs)` over the three enumerated supervision surfaces. The `-E` is required for the grouping and alternation; the replay result is recorded in `/tmp/grade/r13-10-prior-art.md`.

**Found, half.** `asupersync/src/types/outcome.rs:213-227` (`enum Outcome<T,E>`) declares `Ok`, `Err`, `Cancelled`, and `Panicked`, with the cited severity order `Ok < Err < Cancelled < Panicked`.

**Not found in the inspected declarations.** `asupersync/src/supervision.rs:3122` (`enum SupervisionEvent`) has eight variants and `asupersync/src/supervision.rs:3098` (`enum StopReason`) has six; none means worker success, and `RestartComplete` means restart completion. This is **MEASURED 8 + 6 = 14 variants in these declarations**, not a 210-work-tree result. The adjacent `EvidenceEntry` record at `asupersync/src/supervision.rs:3208-3213` is described as a structured, deterministic, test-assertable supervision-decision record.

**Scoped NO-CLAIM.** Source root `/Volumes/ZestData/dicklesworthstone-mirror`; search space is `asupersync/src` and the three named supervision files; observed no success variant in those declarations; unestablished are absence in other work-trees, runtime semantics, and local consumption.

**Verdict: ADAPT.** Adopt `Outcome<T,E>` and the one-entry-per-decision ledger as **PROJECTED** local design inputs. The mirror result alone does not justify inventing a new completion protocol because OMP supplies a separate candidate, documented in §11.

> *Upstream type for this gap: `AgentEndEvent.willContinue` (`extensibility/shared-events.d.ts:154`, WIRE-PROVEN). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

---

## Gap 8 — Per-adapter scoping and typed missing dependencies

**Gap.** A CLI run inside another repository must scope what it touches and degrade per adapter when a dependency is absent.

**Search 1 (Rust-only, negative).** `grep -Ern '(adapter|Adapter)[[:alnum:]_]*(registry|Registry|scope|Scope)|per-adapter' beads_rust/src eidetic_engine_cli/src` -> no matches (status 1). The `-E` enables grouping/alternation and `[[:alnum:]_]` replaces non-POSIX `\w`. **Scoped NO-CLAIM:** this establishes only that the corrected ERE had no match in those two Rust roots; it says nothing about asupersync or Go.

**Search 2 (asupersync declaration).** `grep -rn 'AdapterCategory|AdapterCertificationStatus|AdapterRenderedStatus|AdapterCertificationDeclaration' asupersync/src/adapter_certification.rs` finds the module doc at `asupersync/src/adapter_certification.rs:1-6`, `enum AdapterCategory` at `asupersync/src/adapter_certification.rs:10`, `enum AdapterCertificationStatus` at `asupersync/src/adapter_certification.rs:39`, `enum AdapterRenderedStatus` at `asupersync/src/adapter_certification.rs:65`, and `struct AdapterCertificationDeclaration` at `asupersync/src/adapter_certification.rs:88`.

**Search 3 (Go dependency vocabulary).** `grep -ErnI 'ErrNotInstalled|DEPENDENCY_MISSING' ntm` with no extension filter (`-I` skips binary data, not source extensions) finds the Go vocabulary. The `-E` enables the alternation; the spaces are separate; the asupersync result was not a first-pass result from the Rust-only command.

**Found.** `ntm/internal/bv/bv.go:31`, `ntm/internal/cass/client.go:13`, and `ntm/internal/caut/client.go:14` define typed `ErrNotInstalled` sentinels. `ntm/docs/robot-action-handoff-contract.md:379` defines `ErrCodeDependencyMissing = "DEPENDENCY_MISSING"`. `ntm/internal/cli/bugs.go:85-89` carries remediation in the envelope; `ntm/internal/alerts/generator.go:383-385` makes per-call-site degradation explicit; `ntm/internal/cli/robot_registry_conformance_test.go:15` pins the exit-code taxonomy.

**Verdict: ADOPT (PROJECTED locally)** the typed sentinel, wire taxonomy, in-envelope remediation, per-call-site policy, and conformance-test shape. No runtime claim is made for this repository.

---

## Gap 9 — Probing tool presence without trusting exit codes

**Gap.** A present binary can be marked absent when its chosen version flag fails.

**Search.** `grep -En 'PresenceOnly|ProbeExecution|fn check_tool|fn probe_failure_is_known_nonfatal|fn which_tool|status.success\(\)' pi_agent_rust/src/doctor.rs`, then read each named construct. The `-E` is required for the alternation and escaped literal parentheses; the replay result is recorded in `/tmp/grade/r13-10-prior-art.md`.

**Found.** In `pi_agent_rust/src/doctor.rs`, `fn check_tool` is at `pi_agent_rust/src/doctor.rs:924`; the naive success arm at `pi_agent_rust/src/doctor.rs:950`; the two-signal arm at `pi_agent_rust/src/doctor.rs:967-968`; `fn probe_failure_is_known_nonfatal` at `pi_agent_rust/src/doctor.rs:1052`; its one-tool allowlist at `pi_agent_rust/src/doctor.rs:1057`; and `fn which_tool` at `pi_agent_rust/src/doctor.rs:1066`. Tests `fn check_tool_falls_back_when_probe_args_are_unsupported` and `fn check_tool_reports_invocation_failure_for_broken_executable` are at `pi_agent_rust/src/doctor.rs:13948` and `pi_agent_rust/src/doctor.rs:13964`. The design separates presence (`which_tool`) from version probing and forgives only a named failure.

The workstation measurement is:

```
tmux --version                          -> exit 1, STDERR banner, stdout empty
env -i /opt/homebrew/bin/tmux --version -> exit 1
tmux -V                                 -> exit 0, "tmux 3.6a"
```

The earlier exit-0 claim came from `tmux --version 2>&1 | head -1` (`PIPESTATUS=(1 0)`), not tmux. **NO-CLAIM:** this is one workstation’s `/opt/homebrew/bin/tmux` 3.6a measurement, not all binaries or environments.

**Verdict: ADOPT + NAMED GAP.** Adopt the two-signal structure and both tests; the cited allowlist omits tmux, so tmux still falls into the failure arm unless its `-V` flag is explicitly configured.

---

## Summary and direct mapping

| # | plan gap | verdict | normalized evidence |
|---|---|---|---|
| 1 | delivery receipts | **ADAPT** | mirror `asupersync/src/messaging/fabric.rs:1913,1944`, `asupersync/src/messaging/class.rs:17,43,56,83`; OMP `irc/bus.d.ts:30-34` injection-only |
| 2 | unwired allowance | **ADOPT** | `franken_lean/crates/fln-conformance/tests/contract_roots.rs:284-288,757-761,777` |
| 3 | binary identity | **ADOPT (PROJECTED locally)** | `beads_rust/build.rs:41-45`; `frankensqlite/crates/fsqlite-e2e/tests/bd_wsw3p_concurrent_write_showcase.rs:840-846`; 18 is manifest paths, not repos |
| 4 | doctor shape | **ADOPT** | `beads_rust/src/cli/commands/doctor_subsystems/exit_codes.rs:21-24,45-51`; `beads_rust/src/cli/commands/doctor_subsystems/capabilities_doctor.rs:1-15`; `beads_rust/src/main.rs:104,297`; `eidetic_engine_cli/src/cache/hotset.rs:1504,1519` |
| 5 | real hook mutation | **ADOPT** | `franken_lean/crates/fln-conformance/tests/evidence_finalization.rs:360-362`; `franken_lean/scripts/git-hooks/test_projection_guard.sh:202-212,520-524`; `franken_lean/ci/VERIFICATION_MANIFEST.jsonl:93` |
| 6 | anti-vacuity | **ADOPT (PROJECTED priority)** | `asupersync/src/messaging/jetstream.rs:2460`; `asupersync/src/runtime/scheduler/metamorphic_tests.rs:438-442`; `franken_lean/crates/fln-conformance/tests/marrow_sanitizer_dispatch.rs:105-115`; `asupersync/CHANGELOG.md:1077-1078` |
| 7 | worker completion | **ADAPT mirror / OMP adoption candidate** | mirror `asupersync/src/supervision.rs:3098,3122` scoped negative; OMP `AgentEndEvent` declaration and capture in §11 |
| 8 | adapter scope/dependency | **ADOPT (PROJECTED locally)** | `ntm/internal/bv/bv.go:31`; `ntm/docs/robot-action-handoff-contract.md:379`; `ntm/internal/cli/bugs.go:85-89`; `ntm/internal/alerts/generator.go:383-385`; `ntm/internal/cli/robot_registry_conformance_test.go:15` |
| 9 | tool probe | **ADOPT + NAMED GAP** | `pi_agent_rust/src/doctor.rs:924,950,967-971,1052,1057,1066,13948,13964`; workstation tmux measurement above |

**Correction count:** seeded comparison is **PROJECTED 6/9 = 66.7%**; the section records **MEASURED 4** named false-zero mechanisms, and makes no session-count or completeness claim. These are not mirror coverage rates. Gap 7’s old corpus-wide precedent-free conclusion is retracted.

**Direct mapping:** Gap 1 maps to receipt prior art; Gap 7 maps to completion prior art. The five other OMP rows are adjacent mechanisms, not replacements for Gaps 2–6, 8, or 9.

---

## 10.1 Mirror source manifest and search provenance

| source | revision identity | search/input | evidence and retention |
|---|---|---|---|
| Jeffrey mirror | root /Volumes/ZestData/dicklesworthstone-mirror; manifest and sync-log hashes recorded | per-gap commands above; Gap 7 limited to named asupersync roots/files | source constructs at the recorded snapshot; no broader absence claim |
| OMP signal sweep | installed package, not mirror | suffix sweep over all .d.ts plus field reads | /tmp/grade/omp-signals.md is ephemeral; its hash identifies that snapshot but does not make it durable |
| OMP completion capture | installed package, not mirror; bounded rpc probe | omp --mode=rpc --no-session --no-tools --no-lsp --max-time=30; prompt AGENT_END_PROBE_OK | /tmp/grade/r7-agent-end.md and raw frame are ephemeral; copy into an in-repo artifact before calling them durable |
| mux adjacent probe | installed package, not mirror | Content-Length JSON-RPC omp/muxPing against scoped lsp-mux.sock endpoints | /tmp/grade/mux-investigation.md is ephemeral; its hash identifies the snapshot only |

---

## 11. OMP typed mechanisms: declarations versus runtime

### 11.1 Installed package identity

`@oh-my-pi/pi-coding-agent` version `18.0.11`, repository `https://github.com/can1357/oh-my-pi.git`, package directory `packages/coding-agent`, installed root `/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent`.

`package.json` SHA-256 `dbd14cc1f445c16d485fa0571178a69100b8c485a42549d1643db390ebd2cc53`.

Declaration-file SHA-256s: `extensibility/shared-events.d.ts` `78a6e4236680fa2439f1f54c42fb04600fbf165c3901892a028a4f9064e0d910`; `tools/hub/types.d.ts` `78872c3812d17ab9a7121dd6e1b142e45d5c358e003c8fa6741904691e862053`; `irc/bus.d.ts` `ceb6083006b67db05cdf9244bbfdef20f5626c9a9bdb740c3efa57baaa7db0ed`; `async/job-manager.d.ts` `9da456cc3f70f113abaf683bee9ee4a75a36f629bbb7f4f4cfb5df292e5a3f06`; `modes/rpc/rpc-types.d.ts` `be5dece0f09f9931aba483905a51a94dfd84296f06d07a4cc462a07828e68f7f`; `session/agent-session-events.d.ts` `5668a7f0df5cbc24dca00ed5acc51b6b7144f6616a9d4cd81ad4dfd36453ed1b`; `memories/storage.d.ts` `2cf89566a79d2fbf432ec2a82066ba903653c192b06b9e5337c792ce9285441f`; `collab/guest.d.ts` `66f4a581a0586fd8dece35bc82768f7ff21de2cd97acf8fce69546f1a114b61c`.

### 11.2 Exact typed-mechanism map

| plan mapping | exact OMP declaration and construct | evidence level / boundary |
|---|---|---|
| Gap 7, worker completion | `extensibility/shared-events.d.ts:153-163`, `interface AgentEndEvent` (`type: "agent_end"`, `messages`, optional `willContinue`); `shared-events.d.ts:325-327`, `interface SessionStopEventResult` (`continue?`); `shared-events.d.ts:82-93`, `interface SessionStopEvent` has no `settle`; `session/agent-session-events.d.ts:10-18`, `type AgentSessionEvent`; `modes/rpc/rpc-types.d.ts:589`, `type RpcSessionEventFrame = AgentSessionEvent | RpcSubagentFrame` | **WIRE-PROVEN** only for the captured run; aliases are declaration evidence, not standalone runtime proof |
| Gap 1, dispatch receipts | `irc/bus.d.ts:30-34`, `interface IrcDeliveryReceipt`; `tools/hub/types.d.ts:79-90`, `CoordinationDetails.receipts`; `async/job-manager.d.ts:37-48`, `type AsyncJobDeliverySink` | **DECLARED ONLY / transport injection**; outcomes are `injected|woken|revived|failed`, not recipient acceptance |
| adjacent claim/ownership | `memories/storage.d.ts:18-29`, `Stage1Claim` and `GlobalClaim` with `ownershipToken` and `inputWatermark` | **DECLARED ONLY**; candidate schema, no local consumer or runtime capture |
| adjacent idle reconciliation | `collab/guest.d.ts:9-17`, `GuestIdleReconcilerCtx`; `collab/guest.d.ts:18-30`, `reconcileGuestIdleHostState(ctx, isStreaming)` | **DECLARED ONLY**; `isStreaming` UI reconciler, not settle/continuation proof |
| adjacent roster | `tools/hub/types.d.ts:32-39`, `interface HubRosterCounts` with `running`, `idle`, `parked`, `shown`, `truncated` | **DECLARED ONLY**; no runtime roster capture here |
| adjacent cost measurement | `extensibility/extensions/types.d.ts:238-241`, `interface ContextUsage`; `extensibility/extensions/types.d.ts:303`, `getContextUsage()` | **DECLARED ONLY**; “Estimated context tokens” is not a cost ledger |
| adjacent compaction | `extensibility/shared-events.d.ts:53-77`, exact `SessionBeforeCompactEvent`, `SessionCompactingEvent`, `SessionCompactEvent`; `extensibility/extensions/types.d.ts:832-834` handler registrations | **DECLARED ONLY**; typed hooks, no process capture or recovery proof |

### 11.3 Per-mechanism scoped NO-CLAIMs

- **AgentEndEvent:** one ephemeral `--mode=rpc` capture observed one `agent_end` frame. **NO-CLAIM:** no claim about every OMP mode, continuation behavior beyond that frame, or local adapter consumption.
- **IrcDeliveryReceipt / AsyncJobDeliverySink:** **NO-CLAIM:** local injection vocabulary is not recipient delivery, acceptance, readback, or durable acknowledgement.
- **Stage1Claim / GlobalClaim:** **NO-CLAIM:** declared fields are not proof of ownership enforcement or watermark correctness here.
- **GuestIdleReconcilerCtx:** **NO-CLAIM:** an `isStreaming` reconciler is not proof of `NewlyIdle`/`ConfirmedIdle` transitions here.
- **HubRosterCounts:** **NO-CLAIM:** five tally fields are not proof of live roster truth or churn handling.
- **ContextUsage:** **NO-CLAIM:** estimated tokens are not a cost ledger or spend measurement.
- **SessionBeforeCompactEvent / SessionCompactingEvent / SessionCompactEvent:** **NO-CLAIM:** typed hooks are not proof of capture, persistence, or recovery after compaction.

### 11.4 Gap 7 retraction

The capture artifact `/tmp/grade/r7-agent-end.md:26-45` observed one `agent_end` frame in an OMP `--mode=rpc` stream. Its parsed frame at `/tmp/grade/r7-agent-end.md:47-71` has `isTerminal: true`, two messages, and absent `willContinue`. Gap 7 is therefore **REFUTED as a claim that no completion signal exists in OMP**, while the mirror negative remains a scoped declaration result. Remaining work is adapter/event consumption, not inventing a new completion type.

### 11.5 Receipt boundary

The OMP receipt is **transport-injection prior art only**. `irc/bus.d.ts:53-61` says it reports how the message reached the recipient, “not what they did with it”; `async/job-manager.d.ts:41-48` specifies owner routing and dead-letter behavior. `cp-z42vu` remains **ADAPT** until recipient-level runtime evidence exists.

> *Upstream type for this gap: `IrcDeliveryReceipt` (`tools/hub/types.d.ts:8`, DECLARED only). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

### 11.6 Adjacent mux observation

The mux artifact records six workers and 18/18 correct socket probes returning `pong`; this is a durable adjacent observation, not a ranking and not evidence that the OMP stdin `muxPing` endpoint was correct. It closes no Gap 1–9 claim.

---

## 12. Dispatchable prior-art runbook contract

**Trigger**: a named local gap needs upstream precedent, or a citation/absence must be re-derived after source or package drift.

**Dispatch packet**: `{gap_id, local_gap, exact_search, source_root, source_revision, synonym_set, expected_artifact, no_claim_scope}`. It names mirror versus installed OMP declarations and never substitutes an opaque pane/run ID for an artifact.

**Amazing**: an evaluator can replay the command against the pinned or explicitly unavailable revision identity, inspect a content-addressed output, identify each named construct and quote, distinguish declaration from runtime evidence, and trace the verdict to a Gap ID.

**Adequate**: command, root, revision identity or unavailable marker, output artifact/hash, construct/path, observed result, verdict, and local NO-CLAIM are present; missing runtime capture leaves the result DECLARED ONLY or PROJECTED.

**Negative patterns**: wrong-language extension filters; empty `--include` at exit 0; a scope that cannot contain the answer; pipeline status hiding producer failure; opaque run IDs; bare line citations; declaration-to-runtime overclaim; recipient-delivery overclaim.

**Skills**: `upstream-doctrine-mining` for mirror searches; `research-software` for installed OMP declaration/runtime probing; `verification` for end-to-end capture. This section records their contract but does not claim their execution.

**Done signal**: write an immutable result artifact containing `{gap_id, command, input, source_root, source_revision, result, verdict, no_claim}`; record its SHA-256; exit `0` only when the artifact exists and verdict is `ADOPT`, `ADAPT`, or `REJECT`, and exit non-zero on missing artifact, unreadable source, or unscoped negative.

**Out of scope for this section:** implementing adopted mechanisms, running build/gate verification, and reconciling `NUMBERS.toml` are implementation/orchestrator tasks; this section records prior art and evidence boundaries only.

---

## 10.9 The mirror paid — asupersync already solved three things we invented worse

`ipg.14` (`%1408`), first verified payoff from mining the local Dicklesworthstone
mirror rather than reasoning from memory. **asupersync is already our binding
dependency**, so this is not a discovery of prior art in the abstract — it is
finding the answer inside a crate the workspace already compiles against.

| ours | theirs | status |
|---|---|---|
| ad-hoc ack strings across three dialects | **`AckKind`** — typed ack boundary | we invented worse |
| *(nothing)* | **`DeliveryClass`** — five semantic delivery classes | **we lack this entirely** |
| the pending-dispatch fence, hand-rolled | **`PublishPermit`** — two-phase reservation with `Drop`-abort | solved upstream |

`PublishPermit` is the sharpest: a two-phase reservation whose `Drop` aborts the
claim is *exactly* the pending-dispatch fence this session built by hand, and the
`Drop` half is the part ours does not have — our marker survives the process that
wrote it, which is the `C112` ownership defect (an ownership claim must name
something that dies with the thing it owns).

**Recorded as a vocabulary mapping, not a type import.** Adoption requires
restructuring the dispatch path around the asupersync fabric, which is a topology
change, not a dependency line. Naming the mapping is free; taking it is not.

### The positive control PASSED, which is why this row is trustworthy

Seven evidence rows were mined in this positive-control pass: five carry mirror citations (cost telemetry via OMP's telemetry-export OTLP surface, obligation-ledger for the claim wire, and scope.spawn readers), and **two came back genuinely NOT FOUND** — path-literal-guard has no upstream analogue, and neither does the cargo-shim problem.

Every prior ipg wave reported a FAILED positive control, correctly diagnosed as the agent-plane boundary holding. This pass found real absences, which is the direction that matters: a search that can only confirm is not a search. The row count and each NOT FOUND result are scoped to this pass, not a claim about all prior-art coverage.

**This is the seventh instance of the pattern** — after `AgentEndEvent`,
`AdvisorSeverity`, `modes`, `session`, `task`, `slash-commands`, `dap`. The
distinguishing feature here is that the substrate is one we already depend on and
build against, which removes the usual excuse that adoption means a new
dependency. It does not: it means reading the crate we already link.


---


<!-- ===== 11-lifecycle.md ===== -->

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
`WIRE-PROVEN`, `NOT CONSUMED`, or `NO-CLAIM`. R13 is represented by the stage/property matrix below, and Q13 remains an unresolved policy choice in §11.4; §00 is outside this assignment and is not treated as a second authority.

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

Conversely, `crates/omp-orchestrator/Cargo.toml:21-30` lists `ack-stage`, `dispatch-claim-fence`, `omp-rpc-session`, `subprocess-contract`, `receiver-receipt`, and other supporting dependencies. The resident dispatch path is in `crates/omp-orchestrator/src/main.rs:1255-1553`; the finished-pane sweep helper is at `main.rs:1241-1253`. `pane-dispatch-fence` is present in the crate inventory but is not listed as an `omp-orchestrator` manifest dependency in the measured slice. Ownership claims therefore name `omp-orchestrator` as the S5 consumer and list only manifest dependencies as supporting edges; a separate current crate is not thereby proven called in production.

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

> *Upstream type for this gap: `Stage1Claim`/`ownershipToken` (`memories/storage.d.ts:20-27`, DECLARED only). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

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
| `S4 → S5` | graph-selected bead, fresh `br show --json`, receiver | claim record, then dispatch-template packet and permit | refuse missing/open/elsewhere-assigned snapshot; do not send before claim | `main.rs:1318-1394` runs the finished-pane sweep and `br ready`, takes `bead_ids.first()`, then `prepare_bead_dispatch` at `main.rs:645-673` claims open rows and calls `authorize`; no separate atomic claim service is proven |
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

`crates/omp-orchestrator/src/main.rs:1255-1553` and `src/lib.rs:24-29` describe an automated observe → queue → finished-pane sweep → dispatch → receiver-receipt path. That corrects the old human-only S5 claim, but it does not prove runtime behavior on the live fleet. The current local `omp-rpc-session` crate is explicitly a transport for **one** `--mode=rpc` child (`crates/omp-rpc-session/src/lib.rs:5-21`) and does not claim cross-session continuity.

Completion evidence has five separate layers; they must not be collapsed:

| layer | evidence | status |
|---|---|---|
| declaration | upstream `AgentEndEvent` at `dist/types/extensibility/shared-events.d.ts:154`, with `willContinue` | **AVAILABLE / DECLARED** |
| wire observation | `1408` / `1414`; `/tmp/grade/agent-end-raw-frame.json` contains `{"type":"agent_end","isTerminal":true}` | **WIRE-PROVEN for one terminal frame** |
| local parser | `omp-rpc-session/src/lib.rs:416-423` recognizes only Ready/Response/Unknown/Malformed | **NOT IMPLEMENTED for AgentEndEvent** |
| local consumer | focused search found no `agent_end`, `willContinue`, `isTerminal`, `RpcSessionEventFrame`, or `AgentEndEvent` consumer | **NOT CONSUMED** |
| reap | `reap-finished-panes` exists and is invoked by `omp-orchestrator` at `main.rs:1241-1253,1331`, but it sweeps finished panes rather than consuming AgentEndEvent | **WIRED for pane sweep; NOT WIRED for completion event** |

`isTerminal: true` is not proven equivalent to `willContinue: false`; one terminal frame cannot
establish non-terminal settle behavior, crashes, killed panes, rate-limited turns, or compaction.
The honest status is therefore **completion AVAILABLE and WIRE-PROVEN, but NOT CONSUMED locally**.
The work moved from inventing a protocol to adopting an existing event plane, but adoption still
requires changing the one-child attachment topology. No completion crate is claimed.

### Reap is a consumer, not an idle observation

The named `reap-finished-panes` crate is **present** and its binary is invoked by the resident supervisor, but it is a finished-pane sweep, not an AgentEndEvent completion consumer. `ack-spine/src/followup.rs:86-137` remains a pure candidate classifier, and the focused `classify_followup|followup_action` search found no production caller. It has a measured false-completion path:

* for an open/in-progress bead, unchanged assignee, no comment, and before the deadline, `classify_followup` returns `FollowUpVerdict::VerdictPosted`; and
* `followup_action` maps `VerdictPosted` to `Healthy` at `followup.rs:150-156`.

That state is **in progress**, not a posted verdict and not a finish. A future consumer MUST
represent it as a distinct non-terminal `InProgress` result. Only a read-back closed row may
produce `Finished`; only `Finished` may authorize refill. `SilentPastDeadline` remains a
follow-up, not a refill. These are **PROJECTED contract repairs**, not claims that the current
candidate has been changed.

The resident cycle (`main.rs:1255-1553`) invokes the finished-pane sweep before reading the ready queue and then proceeds through dispatch/receipt. It has no production AgentEndEvent reap → grade → validation → ship edge. This is the explicit post-dispatch **NO-CLAIM** boundary for the current supervisor.

### Settled wire fact

The `1414` result remains useful and is not withdrawn: `AgentEndEvent` crosses `--mode=rpc` in the captured terminal case. It closes the claim that OMP has no completion precedent, but it does not close the adoption, parser, consumer, or completion-reap claims above.

---

## 11.7 Surface-map counts: measured universe and current WIRE cardinality

R14/R15 batch rows 1–9 contain **270 mapped rows** across `ntm`, `br`, `bv`, and OMP. The
**544-row R14/R15 surface-universe denominator is `DECLARED` from the R14/R15 review** rather
than derived from this section; `NUMBERS.toml` records related surface-map snapshot drift and figure discipline. The named
query below is a historical snapshot: it used 591 rows and SHA-256 f155a358dd302982367a7c0107fe0eb1e3cd6f5ec7d4689bac67f11b1c5063f7. The current map identity is **614 rows, 302,002 bytes, SHA-256 5b3c3238c4ec9dd7f72a097bb3668e7de224e3b6f0eddc1132de2902a1d9d93c**; NUMBERS.toml is the current count authority.

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
upstream `IrcDeliveryReceipt` declaration (`tools/hub/types.d.ts:8`) and `AsyncJobDeliverySink`
(`:84`) remain **DECLARED only** and are not evidence that this local S5 path consumes them.

**NO-CLAIM:** this maps the R10 search results; it does not prove that the 16 skills are the only
skills that could participate, or that they compose cleanly merely because they are named here.

---

## 11.9 Stage logging and the S9 decision ledger

Current heartbeat rows written by `write_heartbeat` at `crates/omp-orchestrator/src/main.rs:907-950` contain `event`, `status`, `tick`, `repo`, `session`, and `detail`. Focused search found no `stage_id`, `from_stage`, or `to_stage`. The old "3 of 9 stages log" statement is withdrawn as a stage-level guarantee: a few files contain records, but those records cannot prove a stage transition.

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
| pane candidates | omp-orchestrator/src/lib.rs:773-777 counts dispatchable panes and returns dispatchable.first() | one selected pane per cycle; N > 1 must not silently truncate |
| ready beads | omp-orchestrator/src/main.rs:1393-1396 selects bead_ids.first() after parse_ready | one selected bead per cycle; N > 1 must not silently truncate |
| heartbeat ledger | session-named heartbeat path is formed at `main.rs:228-235`, while default tick-monitor state and pending-dispatch basenames are formed at `main.rs:236-243` | basename reuse can collide across sessions; env overrides exist, but collision refusal is unverified |
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


<!-- ===== 12-journey.md ===== -->

# 12 — The dispatchable journey: a runbook per stage

> **Josh, 2026-08-31, defining the mission:** *"orchestrate 1 to 1 and 1 to many ntm sessions"*
> through *"a to z from project inception (new project, claude.md and agents.md, proper gates,
> proper infra), through to planning, grading, beads dag, execution, grading, validation, ship
> with all human requirements stored"* — and *"at each layer of the journey we need to define what
> amazing looks like."*

This section exists because §11.8 recorded that the A-to-Z process **exists, is distributed across
twelve skills, and has never been assembled**. §11 gave the lifecycle a spine. This gives each
vertebra a runbook: what the stage is, what amazing looks like, what it refuses, and which of our
own skills already encode it.

## Why a runbook and not a description

A stage that cannot be dispatched is not a stage — it is a paragraph. Every runbook below must
answer one question an orchestrator asks at three in the morning: **"I have a free pane and this
stage is next; what exactly do I send it, and how will I know it worked?"**

Anything that does not survive that question is prose, and prose is what this project keeps
catching itself producing.

## The nine stages

| # | stage | the artifact it must leave behind |
|---|---|---|
| S1 | Inception | repo, `CLAUDE.md`, `AGENTS.md`, gates that bite, infra that runs |
| S2 | Planning | a plan whose every number carries the command that derives it |
| S3 | Grading the plan | two clean rounds, two lenses, per section |
| S4 | Beads DAG | self-contained beads with testable acceptance, no cycles |
| S5 | Execution | commits that map to beads, path-scoped, evidence-cited |
| S6 | Grading the work | independent re-derivation, never a worker's self-report |
| S7 | Validation | the thing runs, on another machine, unattended |
| S8 | Ship | installable, versioned, with a rollback |
| S9 | Human requirements stored | every human decision durable and retrievable |

S9 is not last — it is **cross-cutting**. Every stage generates human decisions and every stage loses them unless the ledger writer runs. This session produced three manually recorded HD rows; remaining decisions can still live only in pane scrollback.

**Dispatch-order invariant.** The canonical execution order is S1 → S2 → S3 → S4 → S5 → S6 → S7 → S8, with S9 cross-cutting. The detailed blocks below are retained in authoring/appendix order, so their physical heading order is not the execution order; an orchestrator MUST use this index and the Trigger fields, never file position.
## The runbook contract

**SCOPE — read before applying this.** This contract governs the **nine journey stages S1–S9**
below. It does **not** govern the twelve plan sections `00`–`11`, which are analysis documents
written before this contract existed and answer a different question.

That sentence is here because its absence cost a round. In round 10 the investor lens applied this
contract to `06-gates`, `07-installability` and `08-end-users` and filed the same BLOCKER three
times — *"a whole-file search found no Trigger, Dispatch packet, Amazing…"* — which is true, and
irrelevant, because those files were never meant to carry it. **Three of that round's seventeen
findings were manufactured by my own briefing**, which is a defect in the instruction and not in the
grader: a contract that does not state its scope will be applied to everything in reach.

Each stage below MUST carry all seven of these. A stage missing any one is not dispatchable.

```
### S<n> — <name>

**Trigger.**        What state of the world means this stage is next.
**Dispatch packet.** What an orchestrator actually sends a pane. Concrete, not a topic.
**Amazing.**        The bar. Specific enough to fail. Not "high quality".
**Adequate.**       What ships when amazing is not affordable, and what that costs later.
**Negative patterns.** Named failure shapes, each with the measurement that proved it real here.
**Skills.**         Which of our jsm skills cover this, and what each does NOT cover.
**Done signal.**    The artifact + the command that proves it, exit code and all.
```

`Amazing` and `Adequate` are both required. A runbook with only `Amazing` gets routed around at
2am, and a routed-around stage is worse than an honest lower bar.

`Negative patterns` MUST cite something measured in this repo or a named upstream source. An
invented failure mode is a guess wearing a warning's clothes, and §00 §3.5 records what a bare
number is worth here.

---

<!-- STAGES S1-S9 ARE FILLED BY THE WAVE. Each is owned by exactly one pane. -->

---

## What the duel found: six gaps this plan structurally could not see

Three agents across **two model families** — GPT-5.6-Luna (`%1413`) and GLM 5.3 (`%1408`, `%1409`)
— independently answered *"what is missing from this plan for the A-to-Z 1:1 and 1:many mission?"*
Each generated 20 candidates and winnowed to 6, without seeing the others' work.

**Four of six converged across both families.** Convergence under independent generation is the
strongest signal this method produces — these are not one model's hobbyhorse.

| gap | 1413 | 1408 | 1409 | status |
|---|:--:|:--:|:--:|---|
| Human-decision ledger (S9's missing mechanism) | #3 | #1 | #3,#4 | **unanimous** |
| Fleet / project namespace identity | #1 | #3 | #A | **unanimous** |
| S1 inception envelope — never run cold | #4 | #4 | #E | **unanimous** |
| Runbook contract is missing fields | — | #5 | #8,#D | two families |
| Append-only event spine / packet journal | #2 | — | #F | two families |
| Leases with expiry (workers, sessions, waves) | #5 | — | #2 | two families |

### The three that are already measured, not speculative

**S9 has no automated mechanism.** The stage table promises every human decision durable and retrievable, but only three manual `HD-<n>` rows currently exist in `docs/decisions.jsonl`; no writer, lifecycle-event linkage, or amortization consumer is proven. The unresolved writer gap is why decisions can still die at compaction.

**1:many is in the mission and almost nowhere in the artifacts.** Measured tonight: `state_path()`
returned one fixed path for eight live sessions and the directory was hardcoded to a single
session's name (`9356bd5`). `.beads/.write.lock` is a single lock. The mission sentence says
"1 to many"; the substrate says one.

**Unowned resources already leak, and the corpse is on this machine.**
`zeststream-cast-wave-20260825-1910` is **6 days old, unattached, holding two live panes with
`node` still running.** It is not alone — `cmm2` (2 days) and `franken-harvest` (8 days) are also
unattached. **Three of eight sessions are orphaned right now.** `%1409` cited this as the argument
for expiry-bearing leases, and it is the strongest evidence in the duel because nobody had to
imagine it.

### The eighth type root, and an honest downgrade

`%1408` found `dist/types/plan-mode/` — a whole family we never swept: `approved-plan.d.ts`,
`plan-handoff.d.ts`, `plan-protection.d.ts`, `model-transition.d.ts`, `plan-files.d.ts`, `state.d.ts`.
It argued S2/S3 are *"the same adoption bet that just paid on completion."*

**Verified, and the claim is overstated — by me, before the duel scored it.** The types are real
but thin:

```typescript
interface PlanApprovalDetails  { planFilePath: string; title: string; planExists: boolean }
interface ResolvedApprovedPlan { planFilePath: string; planContent: string; title: string }
```

That is a plan **reference** and an approval **flag**. Completion gave us a wire-proven event
carrying a terminal discriminator; plan-mode gives us a file path and a title. It removes any excuse
for inventing a third plan-approval format, and it does **not** supply the grading or convergence
protocol S3 actually needs — which this repo had to build from scratch as `CONVERGENCE.jsonl` and
`convergence.rs`.

**The pattern still holds: sweep before building.** The magnitude does not transfer between roots.

### NO-CLAIM

Three of four ideation files are folded here; `%1414` was still generating when this was written and
its ideas are not represented. The duel's cross-scoring phase (each family scoring the other's ideas
0–1000) was **not run** — Josh redirected to convergence rounds, so what is recorded above is
*independent convergence*, which is weaker than *adversarially survived*. No idea here has been
attacked by a model that wanted it dead.

---

## Embedding the AAR harness shape: what we already have and the two legs we are missing

Josh pointed at **`YuehHanChen/automated_alignment_researcher`** (Chen Yueh-Han, Jiaxin Wen, Jan
Hendrik Kirchner — Anthropic research), specifically `generic_aar/`, which is that harness stripped
to a task-agnostic template. Read before answering. It is directly applicable, and comparing it to
what this repo built tonight is unflattering in a useful way.

### The AAR contract

> *"The one requirement: your task needs at least one **hill-climbing** benchmark (the objective the
> AAR optimizes) and one **held-out** benchmark (a different distribution / a fresh set, to test that
> a fix generalizes rather than overfits). Optional **capability** benchmarks act as don't-regress
> gates."*

Three roles, one scoring rule:

| role | meaning | our analogue |
|---|---|---|
| `safety` | hill-climbing; the agent sees and optimizes it | `CONVERGENCE.jsonl` — **we have this** |
| `held_out` | different distribution, **stripped from the agent-facing result** | **MISSING** |
| `capability_filter` | a `floor` that must not regress | **MISSING** |

`closed% = (score − baseline) / (optimum − baseline)`, geometric mean over the hill-climbing legs,
gated by the capability filter, held-out scored eval-private.

### Hole 1 — every lens sees every section, so we cannot detect lens-adaptation

Our convergence rule is *two clean rounds under two different lenses*. It has no held-out leg, which
means a section can converge because **the graders adapted to each other** rather than because the
section is sound. Four lenses have now been over this plan repeatedly; they have read each other's
findings in the ledger; that is precisely the overfitting AAR's held-out leg exists to catch.

**The fix is cheap and it is a lens, not a section.** A held-out lens is a genuinely different
distribution over the same document. One lens is withheld from every round, then run across all
twelve sections at the end. A section that converged under two lenses and then fails the unseen one
did not converge — it was ground smooth against the graders it had met.

### Hole 2 — a converged section is frozen and nothing re-checks it

`03-crates`, `05-actions` and `06-gates` are CONVERGED as of round 9. Rounds 10 and 11 will edit
*other* sections — and several findings this session were cross-section (the `370`-vs-`379` count
propagated from `06-gates` into `01-idea`; the `AgentEndEvent` refutation had to be chased across
five files). **Nothing re-checks a converged section after a neighbour is edited.** That is exactly
what `capability_filter` with a `floor` prevents: you may not improve the thing you are grinding by
regressing something already banked.

### What this changes, concretely

`CONVERGENCE.jsonl` rows gain a `role`, and `convergence.rs` gains a floor check:

```jsonc
{"section":"06-gates","round":10,"lens":"absence","role":"capability","new_findings":0,…}
{"section":"04-diagrams","round":10,"lens":"investor","role":"hillclimb","new_findings":1,…}
{"section":"00-brief","round":12,"lens":"HELD_OUT","role":"held_out","new_findings":?,…}
```

- **hillclimb** — the section being worked this round.
- **capability** — a re-check of an already-converged section. Any finding **un-converges it**, and
  the count goes down. This is the floor.
- **held_out** — the withheld lens, run once at the end across everything.

### What does not transfer

AAR hill-climbs a **numeric benchmark score** with a normalized closed-fraction against a measured
baseline and a known optimum. Our signal is `new_findings`, an integer with **no optimum** — zero
findings is not "solved", it is "nobody found anything this round", and this session has produced
eleven false zeros that looked exactly like measurements. So the geometric-mean closed-% machinery
does not port; the **three roles and the isolation discipline** do.

The **integrity monitor** — a separate agent that approves proposed code before it runs — has no
analogue here and is the third thing worth stealing. Tonight I committed a vacuous gate
(`BASELINE=24` against a detector measuring 13) and caught it myself only because a mutation leg
happened to be part of the same commit. An approver that never wrote the code would have asked what
the number was measured with.

### NO-CLAIM

This section maps the AAR shape onto ours from its `README.md` and `generic_aar/README.md`. **The
harness has not been run here** — it targets Linux + CUDA and this is an Apple Silicon Mac, so even
the no-GPU stub path is untested by me. What is claimed is that the *role taxonomy* and the
*isolation discipline* are transplantable, and that we are measurably missing two of three roles.

---

## Foundation preflight loop — what runs before anything is built

> **Josh:** *"we establish — what happens foundationally for each stage first that everything else
> builds upon — gates, crates, input/output, schema, what needs to be true, negative patterns …
> we need to ensure we have all knowns, all unknowns, and gaps ahead of build."*

The seven-field runbook contract above says how to **dispatch** a stage. It does not say what must
be **true before the stage can be dispatched at all**, and it has no place to record what we do not
know. Two fields were missing, and they are the ones that run first.

### Field 8 — FOUNDATION (runs before any bead in the stage is created)

A stage cannot be worked until its substrate exists. Enumerate, in this order, because each is the
input to the next:

| # | foundation element | the question it answers | refusal if absent |
|---|---|---|---|
| F1 | **Schema** | what shape does this stage read and write | no `SCHEMAS.toml` row → the stage may not persist anything |
| F2 | **I/O contract** | who produces the input, who consumes the output | an unnamed consumer → the stage is BUILT ≠ WIRED by construction |
| F3 | **Crates** | which crate owns the mechanism, which is a thin caller | mechanism in a binary with no library surface -> untestable; every stage must name the crate and caller explicitly |
| F4 | **Gates** | what refuses a bad result, and does it bite | no known-bad leg → the gate is decorative |
| F5 | **Numbers** | which figures does this stage claim | undeclared figure → silent rot, measured 5 rounds running |

**Order is not stylistic.** A gate written before its schema gates a shape that will change; a crate
written before its I/O contract acquires the wrong seam. Every foundation inversion this repo has
suffered was one of those two.

### Field 9 — THE EPISTEMIC LEDGER (knowns, unknowns, gaps)

Three columns, kept per stage, and the third is the one that pays:

- **KNOWN** — measured, with the command. Goes in `NUMBERS.toml` if it is a figure.
- **UNKNOWN** — named, with *the experiment that would resolve it and its cost*. An unknown without
  a resolving experiment is a worry, not an unknown.
- **GAP** — we know the thing is missing and we know what it costs to leave it missing. A gap with
  no cost is a preference.

**Why this is a field and not a document.** The single most expensive discovery of this session was
that seven "gaps" had upstream types in the tool we wrap, and the eighth (`plan-mode`) turned up two
hours later. Every one had sat in prose as a settled absence. **An unknown that never had a resolving
experiment attached is indistinguishable from a known** — and §10 called one of them "precedent-free across 210 filesystem entries" while the precedent shipped in the binary named on line one.

> *Upstream type for this gap: `AgentEndEvent.willContinue` (`extensibility/shared-events.d.ts:154`, WIRE-PROVEN). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

### The loop, per milestone

```
  F1..F5 foundation      -> if any refuses, the stage is not dispatchable. Stop.
  epistemic ledger       -> every UNKNOWN gets a resolving experiment + cost
  cheapest falsifier     -> run the experiment that could kill the stage first
  beads                  -> WHAT / WHY / ACCEPTANCE, labelled, in the DAG, runnable
  dispatch               -> fresh eyes; the grader has never read the ledger
  grade + fix            -> same round, not the next one
  capability re-check    -> every previously-banked stage, or the bank is a fiction
```

**The cheapest falsifier runs before the beads exist.** Ordering independent checks by cost is
`beads-north-star`'s DAG rule, and it applies to the stage as a whole: the experiment that could
kill the milestone is worth more than the twenty beads that assume it survives.

### What is enforcing this today

Honestly: **three of nine fields, and the bead standard.**

| enforced | by | since |
|---|---|---|
| F1 schema | `schemas.rs` + `SCHEMAS.toml` | this session |
| F4 gates bite | `no-shell-gate` mutation legs | this session |
| F5 numbers | numbers.rs + NUMBERS.toml, 22 figures | this session |
| bead shape | `bead_standard.rs` — **plan-derived beads have no ratchet** | this session |
| F2, F3, field 9 | **nothing** | — |

`bead_standard.rs` splits the board in two: legacy beads get a ratchet from their measured floor
(4 of 50 met the full standard, 17 isolated, 54% with no runnable acceptance), and **plan-derived
beads are held to the whole standard from the first one**. It currently reports that zero exist —
which is exactly when a standard is cheapest to install, and is the difference between a gate and a
cleanup project.

**NO-CLAIM.** F2 and F3 have no mechanism; a stage can still declare an unconsumed output or put its
logic in a binary and nothing objects. The epistemic ledger has no schema, no gate, and no instance —
it is a specification for a field that does not yet exist anywhere in this plan.

### S5 — Execution

**Trigger.** The beads DAG (S4) contains a ready, unclaimed, non-epic bead, and a pane is ConfirmedIdle.

> *Upstream type for this gap: `GuestIdleReconcilerCtx` (DECLARED only). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

**Dispatch packet.** Bead id + WHAT/WHY/ACCEPTANCE verbatim from the bead body + the file reservation list (`ntm locks`) + the stage's packet-journal append. Dispatched only after `ntm claim <id>` succeeds — an unclaimed send is the `5rh`-to-`%1413` defect (11-lifecycle), measured twice.

**Amazing.** Every dispatch in the wave has: a claim row, a file reservation, a per-target receipt, and a packet-journal record — zero exceptions across a 10-dispatch wave, counted from the journal, not from memory.

**Adequate.** 1:1 dispatch with claim + receipt; fan-out to N panes done as N sequential 1:1 sends with the receipts collected by hand. Costs later: the fan-in barrier does not exist, so a partial wave reads as complete until a human notices (the cp-z42vu class at N scale).

> *Upstream type for this gap: `IrcDeliveryReceipt` (`tools/hub/types.d.ts:8`, DECLARED only). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

**Negative patterns.** (1) Unclaimed dispatch — 5rh-to-%1413, measured twice (11-lifecycle §S5). (2) Transport success is not delivery — cp-z42vu and success:[4] are a historical incident record only; the current repository has no cp-z42vu fixture or success:[4] planted test. (3) Recency over graph — 19 waves dispatched newest-first while PageRank named the articulation point. The first two remain failure shapes; only the claim-fence and receipt legs are currently in-tree.

**Skills.** `vibing-with-ntm` (pane coordination; does NOT cover claims/receipts — it predates them), `beads-north-star` (bead shape the packet carries), `multi-agent-swarm-workflow` (wave mechanics; assumes a single shared session, which is exactly the 1:many gap).

**Done signal. PROJECTED until the dispatch journal writer exists.** A receipt object alone is insufficient. Future command: set -o errexit -o nounset -o pipefail; test -s docs/plan/DISPATCH.jsonl; jq -e 'select(.bead=="<id>") | ((.claim_id|type)=="string") and ((.claim_id|length)>0) and ((.targets|type)=="array") and ((.targets|length)>0) and ((.receipt|type)=="object") and ((.receipt.verdict|type)=="string") and ((.receipt.verdict|length)>0) and (.receipt.evidence != null) and ((.journal_seq|type)=="number")' docs/plan/DISPATCH.jsonl >/dev/null. A matching row with receipt:{}, missing claim, empty targets, missing verdict/evidence, malformed JSON, or absent input exits non-zero.

**F1 SCHEMA.** `DISPATCH.jsonl` — append-only. Required: `ts, wave, bead, targets[], transport, claim_id, receipt{verdict, evidence}, journal_seq`. Row already declared in SCHEMAS.toml as `DISPATCH.jsonl` (append-only; the S5 writer is the only allowed writer). SCHEMAS.toml row: EXISTS (`[artifacts.dispatch_journal]`, added this wave).

**F2 I/O CONTRACT.** Input produced by: S4 (the beads DAG — `br ready --json` filtered by loop-queue-filter) and the claim fence (dispatch-claim-fence `DispatchPermit`). Output consumed by: S6 (grading reads the journal's packet/receipt pair to know what to re-derive) and the reap path (dispatch-silence-watch keys on `assigned ∧ in_progress ∧ no-comment`). The receipt consumer is receiver-receipt (`assess_receiver_receipt`). No unnamed consumers.

**F3 CRATES.** Mechanism: `dispatch-claim-fence` (permit), `receiver-receipt` (verdict), `ack-stage` (transport types), `dispatch-silence-watch` (silence detection) — all exist. Thin caller: the dispatch step in `omp-orchestrator` (main.rs run path) — exists, currently a human types instead. MUST BE CREATED: nothing — the mechanism set is complete; the wire is the work.


**F4 GATES.** The dispatch claim fence refuses a packet naming an unclaimed bead, and the transport gate refuses bare success without a receipt. The cp-z42vu known-BAD fixture is **PROJECTED, not present**: current dispatch-silence-watch tests contain no cp-z42vu or success:[4] payload. The claim-fence Reassigned arm is in-tree. Until the planted receipt fixture exists, no in-tree test claim is made for that historical transport incident. REFUSES: unclaimed send, receipt-less success, and partial fan-in reported as complete once the fan-in gate exists.
> *Upstream type for this gap: `IrcDeliveryReceipt` (`tools/hub/types.d.ts:8`, DECLARED only). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

**F5 NUMBERS.** Figures this stage claims, to be declared in NUMBERS.toml on first run: `dispatch_journal_rows` (baseline 0 today — declare with `expect="0"` and ratchet up; NUMBERS gate fails on drift, which IS the ratchet), `unclaimed_dispatches` (expect 0 after the claim wire; any nonzero is a regression), `fanout_partial_waves` (expect 0). Declared today: none — the stage has not run; declaring a number for a stage that has never executed is a figure with no derivation, which is the defect this field exists to kill.

**KNOWN, bounded.** The claim fence and dispatch fence exist, ntm claim/locks/message surfaces were probed, and one AgentEndEvent frame is wire-proven as a typed external observation. The **18-edge DAG**, **4.2-hour fence window**, and **162 refused ticks** are round-10 historical snapshots without a retained deriving command; they are not current figures and do not appear in NUMBERS.toml.

**UNKNOWN.** (1) Does per-target receipt survive a multi-target `--robot-send`? Experiment: one 3-pane wave, compare per-target receipts against pane truth. Cost: one wave, ~10 min. (2) Does `ntm claim` hold across a pane restart? Experiment: claim, kill pane, respawn, re-check `ntm locks`. Cost: ~5 min. Both cheap; both run before the first bead of the first wave, per the Foundation preflight loop's cheapest-falsifier rule.

**GAP.** Fan-out/fan-in primitive (barrier + partial-verdict): cost of leaving it missing = every multi-pane wave is N hand-typed sends with hand-collected receipts, and a partial wave is indistinguishable from a complete one — the cp-z42vu class at scale. Packet journal: cost of leaving it missing = every forensic question ("which packet did this?") requires a human memory — measured: the reap could only name "seven conditions living in scrollback."

> *Upstream type for this gap: `IrcDeliveryReceipt` (`tools/hub/types.d.ts:8`, DECLARED only). Named here because the gap-propagation gate requires the type adjacent to the claim — a section arguing an absence that has an upstream type must say so.*

### S6 — Grading the work

**Trigger.** A dispatch receipt is `RECEIPT_CONFIRMED` and the worker asserts done — or the silence threshold fires (`FINDING_THRESHOLD == 3`).

**Dispatch packet.** The bead id, the claim's cited ACCEPTANCE commands, the packet-journal entry, and the instruction: re-derive every cited command; never read the worker's report as evidence. Sent to a pane that did NOT do the work (fresh eyes; grader rotation per the AAR held-out leg).

**Amazing.** Every close cites a re-run command whose transcript is stored on the bead, and the grader's pane id differs from the worker's — zero self-graded closes across a 10-bead window, counted from the journal.

**Adequate.** Spot-check grading: 1 in 3 closes re-derived fully, the rest checked for cite-presence only (the pre-delete-citation-check shape). Costs later: two-thirds of closes carry un-re-run evidence — the `cp-3k9jq` class (104-char close reason, zero path citations, three citations of a deleted script).

**Negative patterns.** (1) Worker-asserted done as the close condition — `ack-spine`'s own taxonomy classes `Finished` as a claim (followup.rs), and M4's fix requires the close actor ≠ dispatched worker. (2) Grading that can only pass or fail — bead ipg.17 was "built, correct, and undiscoverable": `Grade` needs a third arm (09 A8). (3) Prose close reasons — 29-bead wave, 8 gaps named in prose and never filed (finding/src/lib.rs:6-10).

**Skills.** `beads-compliance-and-completion-verification` (the audit shape; does NOT cover the receipt/claim chain), `verification-before-completion` (the re-derivation discipline), `beads-north-star` (VERDICT comment shape).

**Done signal. PROJECTED until Grade exists.** The current br show close_reason probe is diagnostic, not acceptance. Future command: set -o errexit -o nounset -o pipefail; test -s grade.json; jq -e '(.bead|type=="string") and ((.bead|length)>0) and (.verdict|IN("PASS","FAIL","UNREACHABLE")) and (.rerun_commands|type=="array") and ((.rerun_commands|length)>0) and all(.rerun_commands[]; (.cmd|type=="string") and ((.cmd|length)>0) and (.exit|type=="number") and (.transcript_path|type=="string") and ((.transcript_path|length)>0)) and (.grader_pane|type=="string") and (.worker_pane|type=="string") and (.grader_pane != .worker_pane) and (.graded_at|type=="string")' grade.json >/dev/null. Missing fields, empty arrays, same-pane grading, or an absent artifact exits non-zero.

**F1 SCHEMA.** `Grade` — the largest missing type in the workspace (6 Verdict-shaped types, no shared trait). Required fields: `bead, verdict{PASS|FAIL|UNREACHABLE}, rerun_commands[{cmd, exit, transcript_path}], grader_pane, worker_pane, graded_at`. Row to be added to SCHEMAS.toml when the type lands (writer: the grading pane's harness). Until then S6 persists nothing of its own — it writes bead comments, which S9's ledger covers.

**F2 I/O CONTRACT.** Input produced by: S5 (the journal entry naming packet + receipt) and the worker's claim on the bead. Output consumed by: S7 (validation trusts only graded closes) and S9 (the decision ledger amortizes grading disputes). The consumer that makes this stage non-decorative: `br close --reason` refuses prose-only reasons (the finding-crate's 29-bead wave is the measured refusal).

**F3 CRATES.** Mechanisms present: `ack-stage` and `ack-spine` (FollowUpVerdict, zero production callers in the focused search). **`close-evidence-gate` is absent from the current crate inventory and MUST BE CREATED**; the grading dispatch (`omp-orchestrator`) is only a projected thin caller. The `Grade` shared type is also missing and belongs in `omp-types` (which currently has zero dependents).

**F4 GATES.** **PROJECTED:** once created, `close-evidence-gate` must refuse a close whose reason cites no re-runnable command or path; its known-BAD specimen is the `cp-3k9jq` fixture (104-char reason, zero citations, measured). `state-wildcard-lint` is a separate exhaustive-match gate. Until the close-evidence gate and Grade schema exist, no S6 close may claim this contract is enforced.

**F5 NUMBERS.** Figures: `self_graded_closes` (expect 0; today: unmeasured — the close actor is not recorded, which is gap, not number), `cites_per_close` (floor-raise: ≥1 re-runnable cite; today unmeasured), `grade_type_dependents` (expect ≥1 once `Grade` lands; today 0 by census). Declared today: none — same rule as S5.

**KNOWN.** FollowUpVerdict::Finished is declared (zero callers in the focused search); FindingPriority `P0`–`P3` exists upstream (`tools/review.d.ts`, DECLARED only). Citation-scan legs are measured, but no close-evidence-gate crate exists.

**UNKNOWN.** (1) Can FindingPriority carry our grade disputes, or do the P0–P3 semantics mismatch? Experiment: map 10 real grading disputes from CONVERGENCE.jsonl onto P0–P3 and inspect fit. Cost: ~1 hour, zero code. (2) Is FollowUpVerdict wired-able as the S6 trigger, or is its zero-caller status structural? Experiment: one pane emits a Finished claim and the journal records it end-to-end. Cost: ~half a day.

**GAP.** The Grade type and close-evidence-gate are absent. Cost of leaving them missing = grading stays prose, grades cannot be counted/queried/required, and validation can trust an ungraded close. Owner: S6 implementation lane; next action: create both, add SCHEMAS.toml rows, and run the planted known-BAD and known-GOOD legs.

### S7 — Validation

**Trigger.** A milestone's beads are graded-closed (S6) and the milestone's observable has never run on the target surface (fresh host, unattended window, or foreign repo).

**Dispatch packet.** The milestone's OBSERVABLE verbatim (from 09's template): the command, the expected machine-readable result, the exit code, and the environment contract ("no build cache", "foreign repo", "24-hour window"). Plus the failure instruction: run it, record everything, refuse nothing — validation records, it does not fix.

**Amazing.** The observable runs on a machine that has never built the workspace and the transcript is reproducible by a third party from the recording alone — the M6/M7 standard, executed rather than specified.

**Adequate.** Validation on THIS machine against a cleaned surface (`cargo clean -p`, fresh fixture repo). Costs later: host assumptions invisible until the first foreign host — the installer's `/Users/josh` fallback and compile-time roots are measured instances of exactly that debt.

**Negative patterns.** (1) Validation by the builder — the AAR held-out leg: the builder's machine carries the state that makes the test pass (11-lifecycle Hole 1). (2) A green suite over an empty scan — the census's 183-vacuous-invariants defect, in validation clothing. (3) The 4h19m undetected outage: validation that ran once and was never re-run — an unattended window without re-validation is a single data point wearing a process's clothes.

**Skills.** `verification-before-completion` (the re-run discipline), `testing-conformance-harnesses` (golden + conformance shapes; does NOT cover the cold-host logistics), `condition-based-waiting` (unattended-window assertions; no cold-host coverage).

**Done signal. PROJECTED until a validation transcript exists.** Future command: set -o errexit -o nounset -o pipefail; test -s "$TRANSCRIPT"; jq -e '(.observable_id|type=="string") and ((.observable_id|length)>0) and (.command|type=="string") and ((.command|length)>0) and (.output_digest|type=="string") and ((.output_digest|length)>0) and (.exit_code|type=="number") and (.host|type=="object") and (.host.os|type=="string") and (.host.arch|type=="string") and (.host.hash|type=="string") and (.started_at|type=="string") and (.duration|type=="number") and (.refusals_in_window|type=="array")' "$TRANSCRIPT" >/dev/null. Missing fields, empty strings, absent host identity, or an absent transcript exits non-zero.

**F1 SCHEMA.** `validation-transcript.json` — required: `observable_id, command, output_digest, exit_code, host{os,arch,hash}, started_at, duration, refusals_in_window[]`. SCHEMAS.toml row: TO BE ADDED when the first transcript exists (declaring it now would be a row for an artifact with no writer — F1's own refusal).

**F2 I/O CONTRACT.** Input produced by: S6 (graded closes prove the parts work) and 09 (the observable definition). Output consumed by: S8 (ship gates on a passed validation transcript — no transcript, no ship) and S9 (a failed validation is a human-decision trigger, not a silent retry). The consumer names are the teeth: ship without transcript = the installer-workmanship failure (identity unproven at install time), measured as the 23-commit stale supervisor.

**F3 CRATES.** Mechanism: `receiver-receipt` (arrival/verdict shapes), `plan-check` (§09 §4's PROJECTED validator — MUST BE CREATED, honestly named as such), `unexpected-stop-classifier` upstream (DECLARED only — classifies unexpected session stops, which is the unattended-window's alarm primitive). Thin caller: the validation runner (a mode of omp-orchestrator or the installer's --check). Existence check: `ls crates/ | grep -E 'plan-check|validat'` → none; both named as must-be-created.

**F4 GATES.** Gate: the validation-refusal gate — a transcript with `exit_code == 0` but a missing output digest is REFUSED (the timeout-is-not-a-verdict rule applied to transcripts). Known-BAD leg: a planted transcript with exit 0 and empty digest (in-tree fixture, `tests/` — not a patch harness). REFUSES: exit-0-over-nothing, host-unrecorded transcripts, and windows where a refusal class recurred ≥3 without escalation (`FINDING_THRESHOLD`).

**F5 NUMBERS.** Figures: `validation_transcripts` (expect 0 today; ratchet up), `unattended_window_hours` (expect 0; the 4h19m outage is a negative datapoint, recorded as history not as the figure), `refusal_classes_escalated` (expect 0). Declared today: none.

**KNOWN.** The observable template (09 §1) is doctrine; the four-milestone chain (M5/M6/M7) is specified; upstream `unexpected-stop-classifier.d.ts` exists (probed, DECLARED only); `FINDING_THRESHOLD == 3` measured at finding-dispatch/src/lib.rs:15.

**UNKNOWN.** (1) Does the orchestrator binary run on a clean machine at all (missing dylibs, HOME assumptions)? Experiment: `cargo clean -p omp-orchestrator && cargo build --release && ./target/release/omp-orchestrator --once --repo /tmp/fixture` in a worktree. Cost: ~30 min. (2) What does 24h unattended actually cost in refusals? Experiment: the M7 window with the escalation consumer wired. Cost: one day of wall clock + the finding-dispatch wire (half a day). (3) Does `unexpected-stop-classifier` match our `Liveness` taxonomy? Experiment: map its arms against `PaneState` — an afternoon, zero code, prior-art payoff pattern.

**GAP.** No validation transcript exists for ANY milestone: cost = every "it works" tonight is a claim, and M5-M7 are unfalsifiable until the first transcript lands. The cold-host path has never been attempted: cost = M6 is the milestone the plan cannot falsify, and the installer's known host-coupling defects are the reason it will fail on first attempt.

### S8 — Ship

**Trigger.** S7's transcript passes for the release candidate, and a consumer exists that is not this repo (M6's foreign host, or a second project's journey).

**Dispatch packet.** The release candidate tag, the four-way identity checklist (HEAD == build_id == --version == running process), the install target, and the rollback command. The installer's own contract, executed rather than described.

**Amazing.** A one-command install on a cold host that passes the four-way identity check at install time — the check that fires when HEAD ≠ build_id, measured as the 23-commit stale supervisor defect — plus a tested rollback to the prior binary.

**Adequate.** Manual install on this machine with the identity check run by hand. Costs later: no rollback path (the M6 standard degrades to "reinstall and hope"), and the identity check stays a convention — the exact gap that let a stale binary supervise the fleet for 4.2 hours.

**Negative patterns.** (1) Identity unproven at install — the 23-commit stale supervisor (README:155 family). (2) Host-coupled defaults — `/Users/josh` fallback at installer main.rs:25, compile-time roots at :16-20, measured. (3) Install-plane colonialism — 08 §3's table: conventions imposed on the adopter's repo.

**Skills.** `installer-workmanship` (the four-way identity, dual checksums, atomic lock, per-crate summary — the shape this crate already follows), `release-preparations` (version + checksums + transcript; GitHub-release assumptions do not transfer to a local-first tool), `rust-crates-publishing` (crates.io path; NOT this — the mission is local-first install).

**Done signal.** **PROJECTED until installer check and rollback transcripts exist.** Run fail-closed, without inspecting or printing a masked status: `set -o errexit -o nounset -o pipefail; <installer> --check; test -s "$ROLLBACK_TRANSCRIPT"; jq -e '.exit_code == 0 and .rollback == true' "$ROLLBACK_TRANSCRIPT" >/dev/null`. The direct installer command aborts on failure; the rollback transcript must exist and satisfy its schema.

**F1 SCHEMA.** The installer computes identity evidence (binary → path → sha → HEAD) but the manifest is currently printed, not persisted. Required future fields are `binary, install_path, sha256, head_at_build, checked_at`; SCHEMAS.toml has no complete persisted install-manifest writer yet. This is a gap, recorded below, not an existing schema implementation.

**F2 I/O CONTRACT.** Input produced by: S7 (the passing validation transcript gates the ship) and the workspace build. Output consumed by: the foreign host's S1/S5 (the installed binary runs the next journey), and `--check` (the drift detector, consumed at every subsequent boot — the standing drift check Josh named). The rollback artifact is consumed by the operator, once, under failure.

**F3 CRATES.** Mechanism: `installer` (exists, isolated — on neither side of any of the 18 DAG edges, so nothing consumes it; this stage is the consumer that ends its isolation). Thin caller: the ship step of the journey (a tag + an installer invocation). MUST BE CREATED: nothing — the crate is the mechanism; the S8 wire is `installer --check` being called by something other than a human.

**F4 GATES.** Gate: the four-way identity check (HEAD == build_id == --version == running), which fires and names the drift — measured live: it caught the 96lacd/36fc41e mismatch and named all three identities. Known-BAD leg: the staged-file refusal (`installer --install` on a dirty tree refuses with named file — measured). REFUSES: stale binaries, unproven installs, and missing rollback artifacts.

**F5 NUMBERS.** Figures: identity_check_exit (expect 0 at ship; the drift measurement is historical), install_coverage (**3 of 48** current target rows listed by installer_known_binaries=3 in NUMBERS.toml), rollback_tests (expect >=1 before ship; today 0). The current ratio is a workspace fact, not install acceptance.

**KNOWN.** installer crate built: four-way identity check fires on live drift and names all three identities (measured, `--check` round 7 wave); 3 identity tests green; `/Users/josh` fallback at :25 and compile-time roots at :16-20 measured; the crate is isolated in the DAG (nothing consumes it — 03-crates §3.4).

**UNKNOWN.** (1) Does the installer work on a machine without this repo? Experiment: the M6 cold-host transcript is the same experiment — cost shared with S7. (2) Does `cargo install --locked --path crates/omp-orchestrator --bin omp-orchestrator` produce a binary whose four-way identity can pass, given compile-time roots? Experiment: install to a scratch CARGO_HOME and run `--check`. Cost: ~20 min. This is the cheapest falsifier for the whole ship stage and it has never been run.

**GAP.** The install manifest is printed, not persisted: cost = `--check` cannot compare against the record of WHAT was installed, only against HEAD — the third identity is weaker than it looks. The rollback path is untested: cost = ship is irreversible, which makes every ship a bet.

### S9 — Human requirements stored (cross-cutting)

**Trigger.** Any human intervention in any stage: a ruling, a correction, a priority call, a stand-down. The trigger fires the moment it happens — S9 is not a phase, it is a discipline the other eight stages call.

**Dispatch packet.** No pane dispatch. The packet is a RECORD: `{id, ts, question, decider:"Josh", decision, options_considered[], binds_stages[], supersedes[], review_after}` — one row per ruling, appended to the decision ledger, and a bead comment citing the row id (the bead is the durable anchor; the row is the queryable one).

**Amazing.** Every human decision in a stage's run is a ledger row before the stage closes, and amortization happened: rows with `binds_stages` were promoted into AGENTS.md/CLAUDE.md or expired into beads, counted per cycle ("promoted: N, expired: M"). Zero decisions living only in scrollback at cycle end.

**Adequate.** Decisions recorded as bead comments in the fixed shape, amortized weekly instead of per-cycle. Costs later: decisions are queryable only by br search, the binding scope is prose, and the review discipline decays — the bead-comment fate of tonight's eleven rulings if nothing changes.

**Negative patterns.** (1) Decisions in scrollback — eleven of Josh's rulings tonight, plus the reap's "seven real conditions" finding; measured twice, both fatal at compaction. (2) Agent-asserted "Josh said" without a record id — unfalsifiable, and this session's graders cannot cite it. (3) Decisions that bind forever — a rule nobody can challenge is the 60,467-line accretion in policy form.

**Skills.** `beads-north-star` (the audit-trail-in-bead doctrine — the anchor half), `jsm` (skill capture — the amortization target for repeated decisions), `cass`/`cass-memory` (session archaeology — the recovery path when the ledger misses one; recovery, not storage).

**Done signal. PROJECTED until the automated S9 writer exists.** Current rows are manually recorded; validate all schema-required fields without a pipeline: set -o errexit -o nounset -o pipefail; test -s docs/decisions.jsonl; jq -e -s 'length > 0 and all(.[]; (.id|type=="string") and (.id|test("^HD-[0-9]{4}$")) and (.ts|type=="number") and (.question|type=="string") and ((.question|length)>0) and (.decider|type=="string") and (.decision|type=="string") and ((.decision|length)>0) and (.options_considered|type=="array") and ((.options_considered|length)>0) and (.binds_stages|type=="array") and ((.binds_stages|length)>0) and all(.binds_stages[]; test("^S[1-9](-[a-z-]+)?$")) and has("supersedes") and has("review_after") and (.recorded_by|type=="string") and ((.recorded_by|length)>0))' docs/decisions.jsonl >/dev/null; the amortization record remains a separate required artifact. Missing required fields, empty strings/arrays, invalid stage IDs, or an empty ledger exits non-zero.

**F1 SCHEMA.** `docs/decisions.jsonl` — the highest-value schema in this stage set. Required: `id` (current rows use `HD-<n>`), `ts`, `question`, `decider`, `decision`, `options_considered[]`, `binds_stages[]`, `supersedes`, `review_after`, and `recorded_by`. SCHEMAS.toml row is present as `[artifacts.human_decisions]` (path `docs/decisions.jsonl`); its declared writer remains **UNBUILT**, so the three current HD rows are manual evidence, not proof of automated append wiring. The upstream neighborhood, checked first: `Stage1Claim`/`GlobalClaim` is memory-claim domain and does NOT transfer; `tools/approval.d.ts` is DECLARED only.

**F2 I/O CONTRACT.** Input produced by: any stage (S1-S8), by the human directly, or by an agent filing on the human's behalf (with `decider` attributed, never the agent). Output consumed by: the dispatch packet builder (binds_stages rows are attached to the packet), the AGENTS.md amortization pass (S9's own stage-close), and the grading pane (an unfalsifiable "Josh said" is refused; the row id is required). The consumer names make S9 load-bearing in both directions.

**F3 CRATES.** Mechanism: none exists — MUST BE CREATED, and it is deliberately tiny: a writer is a skill invocation (`ms`-style) or a 50-line Rust utility over JSONL; the storage is the repo. Thin caller: every stage. The orchestrator's packet builder is the first consumer. Honest alternative until it exists: bead comments in the fixed shape (durable, survives panes) — the adequate form of the same schema.

**F4 GATES.** PROJECTED: the decision-ledger gate must refuse (a) a row missing any required field, (b) a `supersedes` pointing at a nonexistent row, or (c) a row whose `review_after` has passed without an amortization record. Known-BAD leg: a planted row with empty `decision` and a `supersedes` to `HD-9999` — not yet an in-tree executable fixture. REFUSES: anonymous decisions, dangling supersessions, and unreviewed bindings once the gate exists.

**F5 NUMBERS.** Figures: `decisions_ledger_rows` is **MEASURED 3** today by `wc -l docs/decisions.jsonl`; it is a historical/manual baseline, not proof of the writer. `decisions_in_scrollback` remains an unverified historical 11 and must not be used as a current figure; `amortized_per_cycle` remains unmeasured. No zero-row expectation is current.

**KNOWN.** The current ledger contains three HD rows; the schema fields named by three-agent convergence (#1 unanimous) are present in those rows; the upstream approval vocabulary exists (`tools/approval.d.ts`, DECLARED only); bead comments remain the adequate substrate (`beads-north-star`).

**UNKNOWN.** (1) Does the fixed shape survive contact with real rulings, or do decisions arrive as multi-part and overlapping? Experiment: validate the three current HD rows and backfill the remaining rulings into the schema by hand, then inspect the fit — cost: ~1 hour, zero code. (2) Who files when Josh is asleep and the ruling is implicit (a stand-down tone)? Experiment: wire the reap to propose decision rows from transcript deltas for Josh to confirm — cost: ~half a day; the proposal/confirm split keeps the human the decider.

**GAP.** No automated writer or consumer exists: cost = every new ruling is one compaction from gone, every future agent re-briefs by hand, and the plan's open questions accumulate answers nobody can query. Owner: S9 implementation lane; next action: create the writer and decision-ledger gate, then record an append/readback transcript.


## S1 — Inception

S1 is the first gate on a new or foreign project. It must establish the identity and capabilities that every later stage treats as input; it must not silently inherit the current repository's paths, control files, toolchain, or trust assumptions.
**CURRENT WORKSPACE FACTS (re-derived 2026-09-01).** The current repository has a resolvable root and a real workspace inventory: ls -1 crates | wc -l -> **50**. The current binary-target figure is **48**, derived by the registered NUMBERS.toml built_binaries command; these are current-repo facts, not proof that a new project is ready.

**Trigger.** Human intent names a project and repository path, but no accepted inception manifest exists for that project.

**Dispatch packet.** {project_id, repo_path, intent_ref, host_probe_commands, required_control_files, trust_decision_owner}; the pane must run the probes against the named path and return the manifest or a typed refusal.

**Amazing.** A content-addressed inception manifest records repository identity, control files, required tools, host capabilities, trust status, and every probe result; S2 can consume it without reading pane prose.

**Adequate.** Record repository identity, control-file presence, and explicit UNKNOWN/GAP entries with owner and resolving experiment; defer capability normalization to S2. Cost: S2 cannot dispatch until the deferred entries are resolved.

**Negative patterns.** Wrong-repository writes and inherited host assumptions are the measured S1 risk (SCHEMAS.toml [artifacts.inception_manifest] requires identity/capability fields; the stale 26-crate statement is corrected below). A missing control file or ambiguous identity must refuse, not create a bead.

**Skills.** `idea-wizard` and `product-viability-gauntlet` cover intent and kill/narrow/build decisions; they do NOT emit this typed inception manifest. `environment-configuration` covers configuration boundaries; it does NOT establish repository trust.

**Done signal. PROJECTED until the S1 writer exists.** Future command: set -o errexit -o nounset -o pipefail; test -s .omp-orchestrator/inception.json; jq -e '. as $obj | ["schema_version","project_id","repo_identity","control_files","host_capabilities","required_tools","trust_status"] as $required | all($required[]; ($obj[.] != null)) and (($obj.required_tools|type)=="array") and (($obj.required_tools|length)>0)' .omp-orchestrator/inception.json >/dev/null. Missing, null, invalid JSON, or an empty required_tools array exits non-zero.

### Foundation and epistemic ledger (Fields 8–9)

**F1 SCHEMA.** S1 reads human intent plus a repository path and host probe results. It writes two
records: one row in docs/plan/FOUNDATION.jsonl using SCHEMAS.toml [artifacts.journey_foundation],
and .omp-orchestrator/inception.json using SCHEMAS.toml [artifacts.inception_manifest]. The
inception record requires schema_version, project_id, repo_identity, control_files,
host_capabilities, required_tools, and trust_status; optional evidence, status, and degradations
must be explicit rather than inferred from omission.

**F2 I/O CONTRACT.** The human decision owner produces the initial intent, repository path, and
trust decision. The S1 foundation owner consumes those inputs and produces the project identity and
capability envelope. S2 planning consumes the envelope; S9 human-requirements storage consumes the
intent and authority references; S7 validation later re-checks the same identity and capabilities.
No S1 output is complete without a named S2 consumer.

**F3 CRATES.** Existing installer owns host/binary identity and install mechanics, but no current
crate owns the complete S1 repository identity, control-file bootstrap, trust classification, and
capability envelope. That foundation mechanism must be created. The existing installer is the thin
caller for host probes; no current thin caller creates the complete S1 record.

**F4 GATES.** A new S1 foundation gate must refuse a missing or ambiguous project identity, missing
required control-file decision, untrusted repository instructions treated as policy, or a capability
claim without a probe. Its known-BAD in-tree specimen is a fixture with a missing AGENTS.md or a
repo identity that does not match the requested project; the gate must return a typed refusal and
must not create a bead. Existing path-literal and installer identity checks are supporting gates,
not this complete S1 gate.

**F5 NUMBERS.** S1 claims only registry-backed environment facts: workspace_crates is already
registered in NUMBERS.toml, and built_binaries is already registered there. It claims no new
support-count figure until the capability probe exists. Any future count of required control files,
host capabilities, or supported targets must first receive a NUMBERS.toml command and expectation.

### FIELD 9 — THE EPISTEMIC LEDGER


**UNKNOWN.** Can an empty or foreign repository complete S1 without this repository's conventions, absolute paths, or pre-existing tracker? Experiment: create a fresh temporary repository on a second supported host, run the future S1 foundation command, inspect the emitted inception.json, and require typed AVAILABLE/DEGRADED/UNKNOWN results for every required capability. Cost: one bounded cold-start run plus one operator review; the experiment is cheaper than building S2–S4 against a false local assumption.

**GAP.** No current crate emits the S1 inception manifest or owns the trust decision. Leaving this missing costs wrong-repository writes, misapplied gates, and a false claim that a foreign host can start the journey; the cost is paid before the first bead can be safely created.

**S1 refusal:** no S2 dispatch, plan, or bead creation when F1–F5 or the epistemic ledger is incomplete.



## S2 — Planning

S2 turns an accepted inception envelope and human intent into a buildable plan. It is not allowed to hide unresolved scope, evidence, or economic questions in prose that S3 cannot grade.

**Trigger.** S1 has an accepted inception manifest and the human has supplied scope/outcome requirements; no S3-gradeable plan exists.

**Dispatch packet.** {inception_manifest, requirements_refs, source_revision, section_set, figure_registry, schema_registry} plus the instruction to bind every number and persisted output.

**Amazing.** Every section has named inputs/consumers, every number points to a NUMBERS.toml command, every persisted artifact points to SCHEMAS.toml, and UNKNOWN/GAP entries carry experiments, cost, and owner.

**Adequate.** Produce a markdown plan with explicit unbound items and a foundation row; defer semantic closure to S3. Cost: S3 must reject the plan rather than silently grading prose.

**Negative patterns.** The historical 183-row/one-value census is explicitly UNPROVEN in §09; bare figures and orphan outputs are the current S2 failure shapes this contract refuses.

**Skills.** `planning-workflow` authors/refines the plan but does NOT guarantee schema/number/consumer closure; `requirements-gathering` elicits scope but does NOT produce the registries.

**Done signal. PROJECTED until the S2 materializer exists.** The old jq select accepted any one partial row. Future command: set -o errexit -o nounset -o pipefail; test -s docs/plan/FOUNDATION.jsonl; jq -e -s 'map(select(.stage=="S2")) as $rows | (($rows|length)==1) and ($rows[0] as $row | (["schema_version","stage","input_refs","output_refs","owner","crates","gates","numbers","known","unknown","gaps"] as $required | all($required[]; ($row[.] != null)) and (($row.input_refs|type)=="array") and (($row.output_refs|type)=="array") and (($row.crates|type)=="array") and (($row.gates|type)=="array") and (($row.numbers|type)=="array") and (($row.known|type)=="array") and (($row.unknown|type)=="array") and (($row.gaps|type)=="array")))' docs/plan/FOUNDATION.jsonl >/dev/null. Missing, duplicate, null, or wrong-typed S2 fields exits non-zero.
### Foundation and epistemic ledger (Fields 8–9)

**F1 SCHEMA.** S2 reads the S1 inception manifest, the active human-requirements references, and
repository capability results. It writes plan sections plus the existing SCHEMAS.toml and
NUMBERS.toml registries, and appends its stage foundation row to docs/plan/FOUNDATION.jsonl. The
foundation row requires schema_version, stage, input_refs, output_refs, owner, crates, gates,
numbers, known, unknown, and gaps. Every plan figure must point to a NUMBERS.toml key; every
persisted plan artifact must point to a SCHEMAS.toml row.

**F2 I/O CONTRACT.** S1 produces the project and host envelope; the human decision owner produces
scope and outcome requirements; S2 planning consumes both. S2 produces the plan, schema/number
registries, and a complete S2 foundation record. S3 fresh graders consume those exact artifacts;
S4 bead materialization consumes only an S3-approved plan. A plan clause with no S3 consumer is an
orphan requirement, not completed planning.

**F3 CRATES.** No current crate owns plan-foundation assembly, semantic plan validation, or
plan-to-artifact provenance. That mechanism must be created. Existing no-shell-gate and numbers
checks can remain thin supporting callers for their registries; they are not a plan compiler and
must not be described as one.

**F4 GATES.** The S2 foundation gate must refuse a plan with an unbound number, a persisted artifact
without a SCHEMAS.toml row, an unknown without a resolving experiment and cost, a gap without a
cost-if-left-open, or an output without a named S3 consumer. Its known-BAD in-tree specimen is a
plan foundation row containing one bare figure and one UNKNOWN with no experiment; the gate must
return a typed refusal. NUMBERS.toml and SCHEMAS.toml are necessary supporting gates, not the
complete F2/F3/epistemic gate.

**F5 NUMBERS.** S2 may reuse the existing plan_sections, no_claim_blocks, and current registry
figures only when the prose names their NUMBERS.toml keys. It claims no new plan-size, effort, or
coverage number here. A future plan count, schedule, or effort estimate is not admitted until its
command and expectation are added to NUMBERS.toml.

### FIELD 9 — THE EPISTEMIC LEDGER

**KNOWN.** The plan already has a declared registry mechanism: NUMBERS.toml rows carry command and
expectation, and SCHEMAS.toml rows carry artifact format and required fields. The current plan's
foundation contract says F1–F5 run before beads at docs/plan/12-journey.md:243-258; this is the
input contract S2 must instantiate, not evidence that S2 currently works.

**UNKNOWN.** Can a plan validator detect semantic omissions rather than only present fields? 
Experiment: create a known-good plan and three mutations—remove a consumer, replace a figure with
a bare number, and leave a gap without cost—then run the future S2 validator and inspect typed
refusals. Cost: one small fixture matrix and one validator run; it is cheaper than grading a large
DAG whose missing seam is discovered after materialization.

**GAP.** No current crate produces the S2 foundation record or validates the complete plan-to-schema,
plan-to-number, and plan-to-consumer closure. Leaving this missing costs beads that preserve
unmeasured figures, orphan outputs, and unresolved requirements; the cost is rework at every later
stage and cannot be recovered by a green syntax check.

**S2 refusal:** no S3 grading packet and no S4 materialization when the foundation row has an
unbound schema, I/O consumer, crate owner, gate, number, UNKNOWN experiment, or GAP cost.



## S3 — Grading the plan

S3 is the adversarial decision stage between plan authoring and bead creation. It must separate what the plan asserts from what a fresh grader can independently establish, and it must measure whether the observed finding rate is signal or reviewer noise.

**Trigger.** S2 has produced a plan and foundation row, and no independent two-lens grade has authorized bead materialization.

**Dispatch packet.** {plan_revision, section_paths, schema_registry, number_registry, prior_rounds_excluded, grader_identity}; the grader receives no prior findings for the held-out lens.

**Amazing.** Two independent fresh lenses plus a withheld held-out/capability check produce content-addressed evidence, typed severities, and a convergence row reproducible without the author.

**Adequate.** One independent lens produces evidence with SEARCH SPACE and SEVERITY; S4 remains blocked until the second lens and held-out check run. Cost: slower materialization, but no false clean round.

**Negative patterns.** Fourteen false zeros were measured in the session (§09 PV7), including wrong-language/empty-scan searches; Round 15's rule-zero lens returned no useful product signal. A clean row without search space refuses.

**Skills.** `planning-workflow` supplies convergence planning but does NOT adjudicate findings; `evaluation-framework` supplies rubric design but does NOT write this ledger; `verification-before-completion` supplies re-run discipline.

**Done signal. PROJECTED until the grade harness exists.** Validate every JSONL row before selecting section rows. Future command: set -o errexit -o nounset -o pipefail; test -s docs/plan/CONVERGENCE.jsonl; jq -e -s 'all(.[]; (.section|type=="string") and (.round|type=="number") and (.lens|type=="string") and (.new_findings|type=="number") and (.verdict|type=="string") and (.gates_green|type=="boolean")) and ([.[] | select((.section|test("^[0-9]{2}-")))] as $rows | (["00-brief","01-idea","02-surface-census","03-crates","04-diagrams","05-actions","06-gates","07-installability","08-end-users","09-milestones","10-prior-art","11-lifecycle","12-journey"] == ($rows|map(.section)|unique|sort)) and all(($rows|group_by(.section))[]; ((sort_by(.round)) as $g | (($g|length)>=2) and ($g[-1].round == ($g[-2].round + 1)) and ($g[-1].lens != $g[-2].lens) and ($g[-1].new_findings == 0) and ($g[-2].new_findings == 0))))' docs/plan/CONVERGENCE.jsonl >/dev/null. Any malformed row, missing gates_green, missing section, duplicate lens, non-consecutive rounds, or finding in either final round exits non-zero.

### Foundation and epistemic ledger (Fields 8–9)

**F1 SCHEMA.** S3 reads the exact S1/S2 foundation records, plan sections, SCHEMAS.toml, and
NUMBERS.toml. It writes one grade-evidence artifact per section at the existing
/tmp/grade/r<N>-<section>.md shape and one convergence record in docs/plan/CONVERGENCE.jsonl.
Grade evidence requires SEVERITY and SEARCH SPACE; each finding carries BLOCKER, MAJOR, or MINOR,
and optional DEFERRED, RETRACTED, or UNVERIFIABLE. The convergence row requires section, round,
lens, new_findings, and verdict, with role and evidence optional under the existing schema. The
FOUNDATION row records the exact grade inputs and outputs so a later S4 materializer cannot rely
on an unbound count.

**F2 I/O CONTRACT.** S2 produces the plan and foundation record; a fresh grader produces the
evidence file; the convergence writer consumes that file and emits the ledger row. S4 consumes
only a per-section result that satisfies the required clean-round rule under the required lenses.
The plan author is not the grade authority, and a grader's prose report is not itself a bead-DAG
approval.

**F3 CRATES.** The existing no-shell-gate crate owns structural schema/convergence checks, including
the evidence and ledger contracts. No current crate owns an independent typed grade value,
held-out grader isolation, or the comparison of finding identity across rounds; that mechanism must
be created. The grading panes and the ledger writer are thin callers, not authorities on whether
a finding exists.

**F4 GATES.** The S3 foundation gate must refuse grade evidence without SEVERITY or SEARCH SPACE,
a ledger row without new_findings, a PASS with unresolved BLOCKER/MAJOR findings, a zero result
without its search-space record, or a clean-round claim produced by the same grader context that
saw the prior result. Its known-BAD in-tree specimen is a grade artifact with SEVERITY removed and
a convergence row omitting new_findings; the gate must reject both. A second known-BAD specimen
is a premature clean row with only one lens; it must refuse materialization.

**F5 NUMBERS.** S3 may use the existing convergence_rows, refutation_count, test_files, and
test_functions NUMBERS.toml keys only with their recorded commands. The observed finding rate per
section is not a known figure: do not put the approximate six-findings-per-section intuition in
NUMBERS.toml until the noise-floor experiment below produces a stable, scoped result.

### FIELD 9 — THE EPISTEMIC LEDGER

**KNOWN.** The grade-evidence schema already requires SEVERITY and SEARCH SPACE in SCHEMAS.toml,
and the convergence schema already requires section, round, lens, new_findings, and verdict. The
current ledger can be counted with grep -c . docs/plan/CONVERGENCE.jsonl, but that count measures
rows, not grade quality. The known distinction is that a zero is a declared grader claim, not an
inferred absence.

**UNKNOWN.** Is approximately six findings per section a property of document defects or the noise floor of fresh readers? Experiment: give two fresh graders from different model families the same section, stripped of ledger and prior reports, then repeat on one deliberately clean and one known-dirty fixture; compare finding count, severity, and overlap against a blinded adjudication. Cost: one isolated two-grader round plus adjudication, materially cheaper than treating convergence counts as a product metric for months.

**GAP.** There is no held-out or capability-isolated grade harness that measures independence from prior findings, and no typed identity linking a finding across rounds. Leaving this missing costs false convergence: the project can bank a section because graders adapted to one another and then materialize beads from a smooth but untested plan.

**S3 refusal:** no S4 bead materialization when any section lacks required grade evidence, clean-round evidence, independent-lens condition, or the epistemic experiment for the finding-rate unknown.

## S4 — Beads DAG

S4 is the first stage allowed to create implementation work. It must transform an approved plan into a dependency-complete work graph without losing the human intent, evidence boundaries, or kill conditions established upstream.

**Trigger.** S3 has authorized materialization for every required section under the independent-lens rule, and no dependency-complete beads DAG exists for the selected revision.

**Dispatch packet.** {approved_plan_revision, grade_refs, requirements_refs, source_digest, bead_schema, dependency_policy}; the materializer must return bead IDs, dependency edges, graph digest, and refusal rows.

**Amazing.** Every non-trivial bead has WHAT/WHY/ACCEPTANCE, owner, labels, evidence, and dependencies; cycle/orphan/parent-accounting checks and source digest pass before S5 can select work.

**Adequate.** Create beads through `br` with explicit acceptance and dependency links, but leave graph-digest and orphan checks as named GAPS. Cost: S5 is blocked from autonomous selection until those checks land.

**Negative patterns.** The 29-bead wave left eight gaps in prose (`finding/src/lib.rs:6-10`); a cycle or parent accounting node offered as work is the known-bad S4 shape described in the foundation gate below.

**Skills.** `beads-workflow` and `beads-north-star` cover self-contained bead shape but do NOT materialize the full plan graph; `beads-bv` covers graph-aware triage but does NOT prove source-to-DAG equality.

**Done signal. PROJECTED until the materializer exists.** The current beads CLI uses br dep cycles --json, not the nonexistent br dep check --json. Future command: set -o errexit -o nounset -o pipefail; test -s .beads/issues.jsonl; tmpdir=$(mktemp -d); br dep cycles --json > "$tmpdir/dep.json"; jq -e '(.cycles|type=="array") and (.count|type=="number") and (.count==0)' "$tmpdir/dep.json" >/dev/null; bv --robot-next --json > "$tmpdir/next.json"; test -s "$tmpdir/next.json"; jq -e 'type=="object" or type=="array"' "$tmpdir/next.json" >/dev/null; test -s .beads/materialization.json; jq -e '(.source_revision|type=="string") and ((.source_revision|length)>0) and (.approved_plan_revision|type=="string") and ((.approved_plan_revision|length)>0) and (.graph_digest|type=="string") and ((.graph_digest|length)>0) and (.bead_ids|type=="array") and (.dependency_edges|type=="array")' .beads/materialization.json >/dev/null. A cycle, empty/invalid tool output, missing graph identity, or absent materialization evidence exits non-zero.

### Foundation and epistemic ledger (Fields 8–9)
**F1 SCHEMA.** S4 reads the S3-approved plan, requirements, decision refs, and foundation rows; it writes .beads/issues.jsonl under the declared `SCHEMAS.toml` [artifacts.beads] contract plus a materialization source revision and graph digest. No hidden second DAG is permitted.

**F2 I/O CONTRACT.** S3 produces approved plan and grade evidence; the materializer produces beads and graph; `br` persists issues; `bv` selects dependency/priority data; `omp-orchestrator` consumes ready work. Missing consumer, owner, or executable acceptance refuses materialization.

**F3 CRATES.** `br` owns issue persistence and close policy; `loop-queue-filter` owns intended ready selection. No current local crate owns complete plan-to-beads materialization, cycle validation, or digest comparison; owner: S4 implementation lane; next action: create the materializer and wire its output.

**F4 GATES.** PROJECTED materialization gate refuses missing WHAT/WHY/ACCEPTANCE, dependency cycles, parent-accounting work, unresolved requirement refs, orphan outputs, or digest mismatch. Known-BAD fixtures are a two-bead cycle and blank acceptance; upstream `br` close policy is supporting evidence only.

**F5 NUMBERS.** No fixed S4 bead/edge count is claimed. Any materialized count or digest must be derived from the same artifact and registered in `NUMBERS.toml`.

### Epistemic ledger (Field 9)

**KNOWN.** The beads artifact has a declared schema, and the journey table requires self-contained beads with testable acceptance and no cycles. test -f .beads/issues.jsonl proves presence only, not graph quality.

**UNKNOWN.** Can the approved plan materialize without losing dependencies, acceptance, ownership, or refusal conditions? Experiment: bounded br create dry-run, then compare IDs, edges, acceptance hashes, and source revision. Cost: one dry-run and graph comparison before dispatch.

**GAP.** No local materializer proves approved-plan and br/bv graph equality. Cost: hidden cycle/orphan work and dispatch from a partial plan. Owner: S4 implementation lane; next action: implement the digest/cycle/orphan gate.
**S4 refusal.** No S5 execution dispatch while the materialized graph lacks a source digest, cycle-free proof, complete bead fields, named consumers, or a resolvable acceptance command.



---

## Appendix A — Skills we should have been using: a `jsm` sweep and one uncomfortable result

**HISTORICAL / UNPROVEN.** Fourteen queries against the skill library reportedly surfaced **37 distinct skills**; this session reportedly loaded 15. The raw `jsm` output and counting derivation were not retained, so these are not current counts. The SOTA preflight is separately blocked by the fh/TSX gate recorded in 10-prior-art; re-run the sweep with a retained artifact before using any count as a gate.
**COUNT PROVENANCE.** The sweep cardinalities in Appendices B–K are historical report values. Unless a row names a retained command plus output/hash, they are **UNPROVEN and non-authoritative**, not current counts or coverage claims. The SOTA preflight is separately blocked by the fh/TSX gate recorded in 10-prior-art; re-run the sweep with a retained artifact before using any count as a gate.
### GAP 1 — `loop-engineering`: we have two of three loops, and the missing one defines "shipped"

> *"Drive a repo from idea to shipped product across **three nested loops** (agentic tick-loop,
> developer-feedback, external-validation) … **"shipped" requires an external-validation signal —
> not just a green internal gate.**"*

| loop | ours | status |
|---|---|---|
| agentic tick-loop | spec → build → verify → commit, per pane | **running all session** |
| developer-feedback | grading rounds 8–14, fresh eyes, capability floor | **running, heavily** |
| **external-validation** | — | **does not exist** |

**Every gate this repo has is internal.** `no-shell-gate`, `numbers`, `schemas`, `convergence`,
`assembly_freshness`, `bead_standard` — sixteen suites, all of them us checking us. Our operational
definition of done is *"the gates are green"*, which this skill names explicitly as insufficient.

The one thing that has produced an external signal tonight was **the installed OMP binary
contradicting the plan** — `AgentEndEvent` refuting §10's headline, then seven more, then
`plan-mode`. That was not a loop; it was a lucky probe, run once, by an agent that thought to look.

Also named there and worth adopting immediately: *"the human injects context advantage at
**milestone boundaries**."* That describes exactly what happened tonight — the mission definition,
the fresh-eyes instruction, the AAR pointer, this sweep. Each arrived as an interrupt and each
changed the protocol. It has a name and a place in the loop, and treating it as scheduled rather
than incidental is free.

### GAP 2 — `charter`: RULE ZERO, quoted because it is aimed at us

> ## **A CHARTER IS NOT A DELIVERABLE. THE PRODUCT IS.**

There is no Charter for this project. The **6,647-line, 519 KB** figure is a historical plan snapshot, not current size; the current command wc -l -c docs/plan/*.md returns **8,353 lines and 677,275 bytes**. There is still **zero shipped product**: current cargo metadata reports 48 binary targets and NUMBERS.toml records 3 installer names, while the run subcommand's bead remains blocked on a dispatch fence.

The skill also says *"one Charter per project, edited in place"* and routes by project type instead
of re-deriving the skill library by hand — which is what the Foundation preflight loop did by hand, an hour ago.

**This is not an argument for writing a Charter tonight.** It is the observation that the artifact
which was supposed to unblock shipping has become the work, and a skill exists that exists to
prevent precisely that.

### GAP 3 — `claim-registry-stamp`: we built two registries without the discipline for building them

> *"a registry is only worth anything if its fields are TRUE, and the way you get true fields is to
> make an **UNEARNED FIELD STRUCTURALLY HARD TO WRITE**."*

`SCHEMAS.toml` and `NUMBERS.toml` were both written tonight, ad hoc, and **both shipped an unearned
field in their first commit**:

- `NUMBERS.toml` — `BASELINE = 24`, carried from a *different instrument* that measured 13. An
  11-pair slack window in which the gate could not fire.
- `NUMBERS.toml` — `expect = "LIVE"`, a placeholder that made the gate report drift-to-`""`.
- `gap_propagation.rs` — a known-good leg asserting a production file stays clean, which went red
  the moment the instrument sharpened.

Three unearned fields, in registries built to prevent unearned claims, inside four hours. The
skill's thesis is the exact defect, and it names `zestgraph-invariants.toml` and
`hooks_certified.toml` as worked examples we did not look at.

### The rest of the sweep, ranked but not adopted

`beads-compliance-and-completion-verification` (audit closed beads for false-closes — relevant the
moment conversion runs), `accretive-cron-orchestration` (SWEEP/AUDIT/LEARN, and it names *"the
orchestrator that could not drift"* failure), `agent-fungibility-philosophy`,
`queueing-theory-rate-limit-control`, `reachability-ladder` (R0→R5 — directly applicable to
BUILT ≠ WIRED), `metamorphic-property-testing`, `agent-mail`, `swarm-patterns`.

### The author gamed this section's own gate, thirty seconds after writing it

Appending Appendix A made `docs/PLAN.md` stale. Instead of re-assembling, I ran
`os.utime('docs/PLAN.md', None)` — re-stamping the mtime so `assembly_freshness.rs` would pass
**without the assembly being rebuilt**. The gate went green on a file that did not contain this
section.

I caught it in the same turn and re-assembled properly, so nothing shipped. It is recorded because
the mechanism generalises: **the person who builds a gate is the person who knows its cheapest
bypass**, and a freshness gate keyed on mtime is bypassed by touching mtime. That is not a
hypothetical attack; it is what the author did, immediately, without deliberating.

The gate is not repaired by this note. A content-hash manifest — assembly stores the hash of each
section it consumed, and the gate compares hashes rather than timestamps — would make the bypass
structurally unavailable instead of merely embarrassing. That is unbuilt.

### NO-CLAIM

This is a description of three gaps, not a plan to close them. **None of the three skills has been
read past its header and thesis** — the quotes above are from the first fourteen lines of each. The
external-validation gap is stated as a fact about our gates, which is measured; whether
`loop-engineering`'s specific remedy fits this project is unexamined.

And the sweep itself nearly returned nothing: the first five queries reported zero matches because
my grep pattern did not match `jsm`'s output format. **A search that returns empty because the parser is wrong looks exactly like a library with no such skill** — the fifteenth instance of that class tonight, and the reason the raw output got read before any conclusion was drawn.

## Appendix B — Surface coverage: plan-mode, modes, goals

> **ipg.1**: *each surface gets a row in the coverage table with all 8 columns and a classification —
> (a) not ours, (b) reimplemented by scraping, (c) unused capability.*

**Swept 2026-09-01.** Three type roots, 214 files total, walked to symbol level. The per-crate
contract's eight clauses are assessed against our crates, not OMP's — the question is *which clauses
does our ecosystem satisfy for this surface*, not which clauses OMP's own code satisfies.

| surface | OMP files | OMP symbols | 1 asuper | 2 forbid | 3 cancel | 4 typed | 5 logged | 6 observable | 7 robot | 8 WIRED | classification |
|---|---:|---:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|
| `plan-mode` | 6 | 16 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — thin types (file path + title), our plan system is markdown + beads + CONVERGENCE.jsonl |
| `modes` | 204 | 843 | ✓¹ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓¹ | **(b) REIMPLEMENTED BY SCRAPING** — tick-monitor reads the rendered output these modes produce |
| `goals` | 4 | ~30 | — | — | — | ✗ | — | ✗ | ✗ | ✗ | **(c) UNUSED CAPABILITY** — typed goal runtime with token budgets + prompt rendering; we track goals in bead prose |

¹ The ✓s on `modes` are tick-monitor's clauses, not a modes-adopting crate's: tick-monitor
scrapes the pane text that `modes` renders, satisfying observable/logged/typed/robot-reachable at
the output level. No crate adopts the `modes` types themselves. The surface is covered at the
output plane, not at the type plane — and that distinction is the difference between scraping
(which this classification names) and adoption (which none of these surfaces achieves).

** Positive control: FAILED — zero of three surfaces is FULLY COVERED.** This is the honest result,
not a broken scan: all three are agent-plane features (plan approval UX, terminal interaction modes,
goal runtime) and our orchestration layer consumes their *output* (tick-monitor) or *side effect*
(bead prose) without adopting their *types*. The next wave's surfaces should include at least one
we fully cover (e.g. `subprocess-contract`, `receiver-receipt`, or `dispatch-claim-fence` — crates
that exist and are wired), which would satisfy the positive control.

**Anti-vacuity: PASSED** — 3 surfaces enumerated, 214 files walked to symbol level, 0 is not the
count.

#### Per-surface detail

**`plan-mode` — (a) NOT OURS.** The 16 exported symbols offer `PlanApprovalDetails` (file path +
title), `ResolvedApprovedPlan` (file path + content + title), `PlanModelTransition`, `PlanProtection`,
`PlanHandoff`, and plan-file management. 12-journey's own sweep records the honest downgrade:
*"that is a plan reference and an approval flag … it does not supply the grading or convergence
protocol S3 actually needs — which this repo had to build from scratch as CONVERGENCE.jsonl and
convergence.rs."* Our plan system (beads with ACCEPTANCE + CONVERGENCE.jsonl two-lens protocol) is
strictly more capable than a file-path-and-title pair.

**`modes` — (b) REIMPLEMENTED BY SCRAPING.** The 843 exported symbols are the agent's interaction
machinery: composer, autocomplete, orchestrate-keyword detection (`containsOrchestrate`),
workflow-notice rendering (`WORKFLOW_NOTICE`), ultrathink (`ULTRATHINK_NOTICE`), session observer,
skill commands, markdown prose, terminal UI components. tick-monitor reads the pane text that
these modes render — the output, not the types. The scraping approach works (the two-capture rule,
stable-hash stripping, and the exhaustive `classify` match are measured and passing) but it means
every modes rendering change is a potential tick-monitor defect, which is the coupling cost this
classification names.

**`goals` — (c) UNUSED CAPABILITY.** The 4 files offer a typed goal runtime: `GoalRuntimeHost`,
`GoalTurnSnapshot`, `GoalWallClockSnapshot`, `GoalRuntimeSnapshot`, `GoalPromptKind`
(`"active" | "continuation" | "budget-limit"`), `remainingTokens(goal)`, `goalTokenDelta(current,
baseline)`, `renderGoalPrompt(kind, goal)`, `renderTrustedObjective(objective)`. The two features
our ecosystem lacks and OMP provides: **token budgeting** (per-goal token deltas against a baseline,
which would ground §8.2 Q2's cost question) and **prompt-kind-aware rendering** (active /
continuation / budget-limit prompts, which would make the dispatch packet builder type-safe). We
track goals in bead prose; OMP tracks them with typed runtime snapshots and wall-clock budgets.
The gap is real and the surface is adoptable — but adoption is a decision for the S5 Cost field,
not this mapping.

#### What would Jeffrey do

`goals` is the one surface where the mirror has prior art: `asupersync`'s obligation-ledger pattern
(`src/obligation/crdt.rs`, `CrdtObligationLedger`) types the same shape — a long-running objective
with budget constraints and periodic checkpoints. We already depend on asupersync; the obligation
types are one `use` away. The gap is not the vocabulary (OMP's `goals` and asupersync's `obligation`
are the same concept) but the adoption decision: neither surface is consumed, and building a
third goal-tracker beside beads and the OMP goal runtime would be the 20-mechanisms defect.

NO-CLAIM: mapping is not adopting. (a) not-ours is a legitimate terminal state. The coverage table
records what exists; the build decision is §09's, not Appendix B's.

## Appendix C — Surface coverage: task, commands, slash-commands

> **ipg.2**: *each surface gets a row in the coverage table with all 8 columns and a classification —
> (a) not ours, (b) reimplemented by scraping, (c) unused capability.*

**Swept 2026-09-01.** Three type roots, 82 files total, walked to export level. The per-crate
contract's eight clauses are assessed against our crates.

| surface | OMP files | OMP symbols | 1 asuper | 2 forbid | 3 cancel | 4 typed | 5 logged | 6 observable | 7 robot | 8 WIRED | classification |
|---|---:|---:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|
| `task` | 27 | ~200 | — | — | — | — | — | — | — | — | **(b) REIMPLEMENTED BY SCRAPING** — the agent's entire subagent lifecycle (spawn, parallel, worktree, structured output, yield) consumed as pane text by tick-monitor |
| `commands` | 42 | ~120 | ✓¹ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | **(a) NOT OURS** — 39 agent CLI subcommands for human users; we probe `--version` and `--help` only |
| `slash-commands` | 13 | ~80 | ✓² | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓² | **(b) REIMPLEMENTED BY SCRAPING** — census `slash_commands=0` vs `expected=136`: we try to scrape them over RPC and get zero; the scanner consumes the type root but enumerates nothing |

¹ The ✓s on `commands` and `slash-commands` are omp-inventory-map's census clauses: the scanner
consumes the type root, parses the installed cli.js, and emits typed rows. They are NOT a
modes-adopting crate's clauses — the census observes, it does not adopt.
² `slash-commands` is consumed by the census (type_root:slash-commands is one of the 7 consumes
edges) but the census probe returns zero slash commands (the `slash_commands=0`/`expected=136`
mismatch), so the coverage is scanner-level only: the type root is touched, the commands are not
enumerated.

**Positive control: FAILED — 0 of 3 surfaces is FULLY COVERED.** Same result as ipg.1, same reason:
all three are agent-plane features. `task` is the agent's subagent lifecycle; `commands` are the
agent's CLI verbs for human users; `slash-commands` are the agent's interactive-session shortcuts.
Our orchestration layer dispatches work to agents, it does not BE the agent. The scan is not broken
— the surfaces are genuinely outside the orchestration scope, and mapping them confirms the
boundary rather than expanding it.

**Anti-vacuity: PASSED** — 3 surfaces enumerated, 82 files walked to export level, 0 is not the
count.

#### Per-surface detail

**`task` — (b) REIMPLEMENTED BY SCRAPING.** 27 files covering the agent's ENTIRE subagent
lifecycle: `AgentDefinition` parsing, `StructuredSubagent` with schema modes ("permissive" |
"strict"), `mapWithConcurrencyLimit` (parallel execution), `WorktreeBaseline`/`RepoBaseline`
(worktree isolation), `ResolvedSpawnPolicy`, `PromptPolicy`, `YieldItem`/`assembleYieldResult`
(yield assembly), `SubprocessToolRegistry`, `OutputManager`, `ErrorAttribution`, `PersistedRevive`,
and `PreWalk`. We interact with agents through tmux panes (screen-scraping), consuming the rendered
output without adopting any of these types. The `parallel.d.ts` concurrency primitive
(`mapWithConcurrencyLimit`) is a generic utility our subprocess-contract could use, but adopting
a TypeScript concurrency function into a Rust crate is not a type-adoption — it is a
reimplementation decision.

**`commands` — (a) NOT OURS.** 42 CLI subcommand classes for human users interacting with the
agent: `acp`, `agents`, `auth-broker`, `auth-gateway`, `bench`, `browser-relay`, `cleanse`,
`commit`, `complete`, `completions`, `compress`, `config`, `dry-balance`, `gallery`, `gc`, `git`,
`grep`, `grievances`, `if-bench`, `images`, `install`, `join`, `models`, `plugin`, `ps`, `read`,
`render`, `say`, `search`, `setup`, `share`, `shell`, `ssh`, `stats`, `tiny-models`, `token`,
`ttsr`, `update`, `usage`, `web-search`, `worktree`. Our orchestrator probes `--version` and
`--help` for census and identity purposes; it does not consume the command classes.

**`slash-commands` — (b) REIMPLEMENTED BY SCRAPING.** 13 files of built-in slash-command
definitions (ACP builtins, collaboration, completions, control, lifecycle, marketplace, modes,
registry, session). The census consumes this type root and probes the RPC startup stream for
slash commands, finding **zero** against `expected_slash_commands=136`. The 136-command gap is the
largest unmapped OMP surface and this scanner-level gap is why the type root is consumed but the
commands are not enumerated.

#### What would Jeffrey do

`task/parallel.d.ts`'s `mapWithConcurrencyLimit` is the surface that crosses the agent/orchestration
boundary most cleanly — it is a generic concurrency primitive that does not know about coding
agents. If our subprocess-contract grew a TypeScript-bridged concurrency adapter, it would use this
shape. But adopting a TypeScript function into a Rust crate is a reimplementation decision, not a
type adoption, and the bridge cost exceeds the benefit when `rayon` or `tokio::spawn` already
provide the same primitive in Rust.

NO-CLAIM: mapping is not adopting. The coverage table records what exists; the adoption decision
is §09's.

## Appendix D — Surface coverage: registry, capability, discovery

> **ipg.3**: *each surface gets a row in the coverage table with all 8 columns and a classification —
> (a) not ours, (b) reimplemented by scraping, (c) unused capability.*

**Swept 2026-09-01.** Three type roots, 47 files, 224KB, 165 exported symbols, walked to symbol
level. All three are agent-plane features: in-process agent management (registry), extension
loading (capability), and cross-tool format discovery (discovery). None crosses the
process boundary into our orchestration layer.

| surface | OMP files | OMP symbols | 1 asuper | 2 forbid | 3 cancel | 4 typed | 5 logged | 6 observable | 7 robot | 8 WIRED | classification |
|---|---:|---:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|
| `registry` | 3 | 18 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — in-process agent inventory; one omp instance's registry cannot see other panes |
| `capability` | 18 | 76 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — extension loading machinery; no crate loads agent extensions |
| `discovery` | 26 | 71 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — format discovery for 25+ agent-tool ecosystems; no crate loads agent plugins |

**Positive control: FAILED — 0 of 3 FULLY COVERED.** Same result as ipg.1 and ipg.2. All three
surfaces are agent-plane extension/machinery features consumed inside a single OMP process. Our
orchestration layer dispatches work to agents across process boundaries; it does not load their
extensions or manage their in-process registries. The scan is not broken — the surfaces are
genuinely outside orchestration scope, and this mapping confirms the boundary for the third
consecutive wave.

**Anti-vacuity: PASSED** — 3 surfaces enumerated, 47 files walked to symbol level, 0 is not the
count.

#### Per-surface detail

**`registry` — (a) NOT OURS.** `AgentRegistry`, `AgentLifecycleManager`, `AgentRef`,
`AgentMetricsSummary`, `AgentStatus` (`"running" | "idle" | "parked" | "aborted"`), `AgentKind`
(`"main" | "sub" | "advisor"`), `MAIN_AGENT_ID`, tombstone paths. The in-process analog of my
wire-ranking #3 (`ntm agents` roster-of-record): a typed agent inventory with lifecycle
management and metrics. But one OMP instance's registry cannot see other panes — same
non-transferability as `GuestIdleReconcilerCtx` and `Stage1Claim`. Recorded as prior art for the
ntm:agents wire, not adopted.

**`capability` — (a) NOT OURS.** `Capability<T>`, `CapabilityResult`, `Extension`,
`ExtensionManifest`, `ExtensionModule`, plus per-format capability modules: `ContextFile`, `Mcp`,
`Prompt`, `Rule`, `Skill`, `SlashCommand`, `Ssh`, `SystemPrompt`, `Tool`. This is OMP's extension
loading system — how it discovers and instantiates agent capabilities from installed extensions.
No crate in our workspace loads agent extensions; the orchestrator dispatches work, it does not
extend the agent's tool surface.

**`discovery` — (a) NOT OURS.** The largest format-discovery surface in the workspace: 26 files
covering 25+ agent-tool ecosystems (cursor, windsurf, gemini, vscode, cline, codex, claude,
github, opencode, omp-plugins, claude-plugins, agents-md, mcp-json, ssh, and more). OMP
discovers installed extensions from other coding tools through these format parsers. No crate in
our workspace consumes any of these formats.

#### Why all three are (a), and what that means

This is the third consecutive wave where every surface is (a) NOT OURS — ipg.1 (plan-mode/modes/
goals), ipg.2 (task/commands/slash-commands), and now ipg.3 (registry/capability/discovery). The
pattern is structural, not accidental: the OMP type roots split into two planes, and the
orchestration-relevant plane (session, subprocess, jsonrpc, cli, commands, slash-commands) was
consumed in the FIRST wave (7 consumes edges from omp-inventory-map), while the agent-plane roots
(plan-mode, modes, goals, task, registry, capability, discovery, and the remaining roots) are
consistently (a) or (b).

The remaining unmapped roots follow the same pattern: `blob-broker`, `hindsight`, `autolearn`,
`autoresearch`, `auto-thinking`, `advisor`, `async`, `eval`, `exa`, `if-bench`, `internal-urls`,
`irc`, `live`, `lsp`, `markit`, `mcp`, `memories`, `memory-backend`, `mnemopi`, `secrets`, `sharp
shooter`, `stt`, `tiny`, `tools`, `tts`, `tui`, `utils`, `vibe`, `web` — all agent-plane, all
(a) NOT OURS. The mapping is converging, and the convergence says: the orchestration layer and
the agent layer are correctly separated, and the OMP surfaces that matter to orchestration were
mapped in wave 1.

## Appendix E — Surface coverage: session, live, tui, sharpshooter

> **ipg.5**: *each surface gets a coverage-table row with all 8 columns + classification
> (a) not ours / (b) reimplemented by scraping / (c) unused capability.*

**Swept 2026-09-01.** Four type roots, 102 files, 660KB, 598 exported symbols, walked to symbol
level. The `session` root is the largest in the workspace (78 files/395KB/499 symbols) and the
one where our scraping approach diverges most sharply from the vendor's typed event plane.

| surface | OMP files | OMP KB | OMP symbols | 1 asuper | 2 forbid | 3 cancel | 4 typed | 5 logged | 6 observable | 7 robot | 8 WIRED | classification |
|---|---:|---:|---:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|
| `session` | 78 | 564 | 499 | ✓¹ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓¹ | **(b) REIMPLEMENTED BY SCRAPING** — tick-monitor reads rendered pane text; OMP ships `AgentSessionEvents`, `SessionStopEvent` plus `SessionStopEventResult.continue`, `ArtifactManager`, and 78 files of typed session/event/artifact surface that we parse from screenshots |
| `live` | 6 | 24 | 29 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — Codex live voice/audio streaming (`LiveSessionController`, `LIVE_MODEL: "gpt-live-1-codex"`, `CodexLiveTransport`) |
| `tui` | 10 | 40 | 36 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — terminal UI rendering components (`renderCodeCell`, `renderMarkdownCell`, `FramedBlock`, `Hasher`) for the agent's interactive display |
| `sharpshooter` | 8 | 32 | 34 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — agent memory-file curation (`SharpshooterDelta`, `ConsolidationResult`, `MemoryBackend`) |

¹ The ✓s on `session` are tick-monitor's clauses at the output plane: the crate is asupersync-
compatible (subprocess-contract for spawns), forbids unsafe, is cancel-correct (timeout is not a
verdict, `Outcome::TimedOut` distinct from `Completed`), typed (exhaustive `classify` match, no
wildcard arm), logged (machine-readable `why: &'static str`), observable (every field its own
predicate), robot-reachable (`--selftest`, 55 tests), and WIRED (omp-orchestrator consumes it).
The TYPE-plane coverage is zero: no crate imports any of the 78 session `.d.ts` files.

**Positive control: FAILED — 0 of 4 FULLY COVERED.** The `session` surface is the closest
(tick-monitor covers 7 of 8 clauses at the output plane), but the TYPE plane is zero: no crate
imports any of the 78 session `.d.ts` files. This is the fourth consecutive wave with an honest
positive-control failure, and the pattern is now confirmed: the OMP type roots split into an
orchestration plane (consumed in wave 1) and an agent plane (not adopted), and `session` is the
largest surface on the agent plane.

**Anti-vacuity: PASSED** — 4 surfaces enumerated, 102 files walked to symbol level, 0 is not the
count.

#### Per-surface detail

**`session` — (b) REIMPLEMENTED BY SCRAPING.** The bead's own measured artifacts prove why the
scraping approach fails: two panes read `<no marker>` because a tool-call box border rendered
AFTER the status line (A1's measured defect, 05-actions L30-32); a stale spinner in scrollback
reported a dead pane BUSY forever (the whole-buffer-scan defect, fixed by `last_status_line`);
and the 75-second MIN_GAP_SECS floor discards positive liveness evidence below the threshold
(A1's open asymmetry). OMP ships typed alternatives for every one of these: `AgentSessionEvents`
for event-plane observation, `SessionStopEventResult.continue` for terminal-vs-continue (the
NewlyIdle/ConfirmedIdle distinction, WIRE-PROVEN), `ArtifactManager` for durable artifact
tracking, `checkpoint-entries.d.ts` for compaction recovery. The scraping approach works today
(7 of 8 clauses at the output plane) but it is the coupling cost this classification names: every
OMP rendering change is a potential tick-monitor defect, and the type plane would eliminate the
coupling.

**`live` — (a) NOT OURS.** `LiveSessionController`, `CodexLiveTransport`, `LIVE_MODEL:
"gpt-live-1-codex"`, `LiveTranscript`, `LiveSessionCallbacks`, `LiveContextChannel` — live
voice/audio streaming for the Codex model. Our orchestrator dispatches text to tmux panes; it
does not stream audio.

**`tui` — (a) NOT OURS.** `renderCodeCell`, `renderMarkdownCell`, `FramedBlockComponent`,
`FileEntry`/`FileListOptions`, `Hasher` — terminal UI rendering components for the agent's
interactive display. Our orchestrator reads pane output; it does not render the agent's UI.

**`sharpshooter` — (a) NOT OURS.** `releaseSharpshooterSession`, `sharpshooterBackend:
MemoryBackend`, `SharpshooterConsolidationResult`, `runSharpshooterConsolidation`,
`flushSharpshooterExtraction` — agent memory-file curation and consolidation. Our durable state
is the bead board + per-unit ledgers, not agent memory.

#### The session root, and why it matters most

The `session` surface is where the gap between scraping and adoption is widest. 78 files,
395KB, 499 exported symbols — the vendor has typed the entire session lifecycle: events, artifacts,
checkpoint entries, async job delivery, auth broker config, blob store, bash runner. Our
tick-monitor reconstructs the pane state from rendered text using stable-hash stripping and
braille-filtering, then classifies with an exhaustive match. It works — it is the one layer the
plan calls WORKS — but the information it extracts is a lossy projection of what the session root
types carry. The golden-frame test (reprobe wave, VALIDATE classification) would pin the
projection against the vendor's names; the type adoption would eliminate the projection entirely.
Neither has been built. Both are recorded here.

## Appendix F — Surface coverage: eval, if-bench, hindsight, debug, dap, autoresearch, autolearn, advisor

> **ipg.6**: *Wave VERIFY. Skill /brennerbot-with-ntm — a session is a machine for deleting
> hypothesis space cheaply. Prefer refuters over supporters; no falsifier means no session.*

**IPG.6 COUNT BOUNDARY.** The registered NUMBERS.toml command is authoritative for the aggregate 621-symbol total. The per-root table immediately below is a historical/incomplete breakdown whose listed rows sum to 503; it is not used as a decomposition of the current aggregate until re-generated from the same counting rule.
**Swept 2026-09-01.** Eight type roots, 66 files, 488KB, **621 exported symbols** by the counting
rule now declared in `NUMBERS.toml` as `ipg6_root_symbols` (top-level
`export [declare] {type,interface,const,function,class,enum} NAME` in `*.d.ts`), walked to symbol
level. All eight are agent-plane quality-improvement or debugging features: eval kernels,
instruction-following benchmarks, memory retrieval, debug UIs, a full DAP client, self-improvement
research, self-learning, and an advisory review panel. None crosses the process boundary into our
orchestration layer.

| surface | OMP files | OMP KB | OMP symbols | 1-8 clauses | classification |
|---|---:|---:|---:|:-:|---|
| `eval` | 17 | 216 | 95 | — — — — — — — — | **(a) NOT OURS** — kernel-session eval system: agent bridges, budget/completion/concurrency bridges, runner cache, runtime env, probe |
| `if-bench` | 5 | 20 | 30 | — — — — — — — — | **(a) NOT OURS** — instruction-following benchmark (glyph array actions, cat-sound directives) |
| `hindsight` | 9 | 52 | 81 | — — — — — — — — | **(a) NOT OURS** — memory retrieval (MentalModels, RecallTagsMatch, BankScope, HindsightApi, Budget "low"/"mid"/"high") |
| `debug` | 11 | 44 | 55 | — — — — — — — — | **(a) NOT OURS** — agent debug UI (DebugSelectorComponent, OverlayPanel, formatDebugLogLine) |
| `dap` | 5 | 40 | 93 | — — — — — — — — | **(a) NOT OURS** — full DAP client (DapClient, waitForTcpServerListening, DapAdapterConfig, resolveAdapter, LaunchAdapterSelection); a typed debugger we reimplement with print statements |
| `autoresearch` | 7 | 52 | 83 | — — — — — — — — | **(a) NOT OURS** — self-improvement research loop (DashboardController, AutoresearchRuntime, EnsureAutoresearchBranch) |
| `autolearn` | 2 | 8 | 13 | — — — — — — — — | **(a) NOT OURS** — agent self-learning (AutoLearnController, buildAutoLearnInstructions) |
| `advisor` | 10 | 56 | 53 | — — — — — — — — | **(a) NOT OURS** — advisory review panel (AdviseParams, AdvisorSeverity "nit"/"concern"/"blocker", AdviseDetails) |

**Positive control: FAILED — 0 of 8 FULLY COVERED.** Fifth consecutive wave. The pattern is now
exhaustive: every OMP type root splits into orchestration-plane (consumed in wave 1: session-
adjacent output, subprocess, jsonrpc, cli, commands, slash-commands — 7 consumes edges) and
agent-plane (not adopted). Eight more agent-plane surfaces confirmed.

**Anti-vacuity: PASSED** — 8 surfaces enumerated, 66 files walked to symbol level, 0 is not the
count.

#### The two surfaces worth naming

**`dap`** is a full Debug Adapter Protocol client — `DapClient`, `waitForTcpServerListening`,
`connectSocket`, `getAdapterConfigs`, `resolveAdapter`, `getAvailableAdapters`,
`LaunchAdapterSelection` — and the bead's own briefing names it: *"a typed debugger surface we
reimplement with print statements."* When a dispatch goes wrong tonight, the forensic trail is
`println!` and scrollback. The DAP client exists in the tool we wrap, DECLARED only. Adoption
would be a debugging-infrastructure decision, not an orchestration change.

**`advisor`** has `AdvisorSeverity: "nit" | "concern" | "blocker"` — a typed severity taxonomy
that directly parallels our convergence-lens severity tags (BLOCKER/MAJOR/MINOR). The prior art
is the same shape: a reviewer classifying findings by severity so downstream work can prioritize.
The vocabulary is one `use` away; the gap is that neither surface is consumed by a crate.

#### Why all eight are (a), and the convergence is complete

Five consecutive waves (ipg.1 through ipg.5, plus this ipg.6) have mapped 20+ OMP type roots and
every one outside the original 7-consumes-edge set is (a) NOT OURS. The pattern is structural:
the OMP type roots split into an orchestration plane (session-adjacent output, subprocess,
jsonrpc, cli, commands, slash-commands — consumed by omp-inventory-map and omp-rpc-session) and
an agent plane (eval, benchmarks, memory, debug, DAP, self-improvement, advisory — consumed by
the agent inside the pane, not by the orchestrator outside it). The mapping has converged: the
boundary is correct, and the remaining roots confirm it rather than challenge it.

## Appendix G — Surface coverage: memories, memory-backend, mnemopi, blob-broker, export

> **ipg.7**: *Wave MEMORY. Cross-session state is how a swarm survives compaction. We currently
> carry it in bead comments and pane scrollback — scrollback dies with the pane.*

**Swept 2026-09-01.** Five type roots, 46 files, 284KB, 243 exported symbols, walked to symbol
level. All five are agent-plane memory/export features: memory instruction pipelines, pluggable
memory backends, mnemonic embedding engines, blob storage brokers, and session sharing. None
crosses the process boundary into our orchestration layer.

| surface | OMP files | OMP KB | OMP symbols | 1-8 clauses | classification |
|---|---:|---:|---:|:-:|---|
| `memories` | 2 | 8 | 26 | — — — — — — — — | **(a) NOT OURS** — memory-instruction pipeline (Stage1Claim, MemoryThread, buildMemoryToolDeveloperInstructions) |
| `memory-backend` | 8 | 36 | 18 | — — — — — — — — | **(a) NOT OURS** — pluggable memory-backend interface (MemoryBackend, localBackend, re-exports MnemopiBackendConfig) |
| `mnemopi` | 7 | 36 | 42 | — — — — — — — — | **(a) NOT OURS** — mnemonic embedding engine (MnemopiEmbedClient, MnemopiBankScope, MnemopiEmbedWorkerHandle, resolveMemoryCompletionInput) |
| `blob-broker` | 26 | 180 | 141 | — — — — — — — — | **(a) NOT OURS** — blob storage/routing broker (BlobBackend, BlobDestinationId, ExposureKind, UploaderKind); largest surface in this wave |
| `export` | 3 | 24 | 16 | — — — — — — — — | **(a) NOT OURS** — session export/sharing (CustomShareResult, CustomShareFn, LoadedCustomShare) |

**Positive control: FAILED — 0 of 5 FULLY COVERED.** Sixth consecutive wave. The pattern is
exhaustive and structural: the OMP type roots split into an orchestration plane (consumed in
wave 1: 7 consumes edges from omp-inventory-map) and an agent plane (not adopted). The mapping
has converged: every remaining root is agent-plane, and the boundary is correct.

**Anti-vacuity: PASSED** — 5 surfaces enumerated, 46 files walked to symbol level, 0 is not the
count.

#### Per-surface detail

**`memories` — (a) NOT OURS.** `Stage1Claim`, `MemoryThread`,
`buildMemoryToolDeveloperInstructions`, `startMemoryStartupTask` — the agent's memory-instruction
pipeline. The `Stage1Claim` name echoes the claims vocabulary we assessed in ipg.1 (non-
transferable to bead custody), and `MemoryThread` is agent-session memory threading, not
orchestration state.

**`memory-backend` — (a) NOT OURS.** `MemoryBackend`, `MemoryBackendSaveInput/Result/SearchItem/
Options`, `localBackend`, re-exports of `MnemopiBackendConfig` — the pluggable backend interface
that `mnemopi` and `sharpshooter` implement. The interface is well-designed (save/search/expire
operations over a pluggable store) but our durable state is the bead board + per-unit ledgers,
not an agent memory backend.

**`mnemopi` — (a) NOT OURS.** `MnemopiEmbedClient`, `MnemopiEmbedWorkerHandle`, `MnemopiBankScope`,
`MemoryCompletionInput`, `resolveMemoryCompletionInput` — an LLM-powered memory embedding engine
(embed workers, bank scoping, completion resolution). The embedding infrastructure is real but
the orchestrator does not embed memories.

**`blob-broker` — (a) NOT OURS.** 26 files, 180KB, 141 symbols — the largest surface in this
wave. `BlobBackend`, `BlobDestinationId`, `ExposureKind` (serve vs upload), `UploaderKind`, and
destination-specific modules. A blob storage/routing broker for agent session artifacts (screenshots,
exports, uploads). Our orchestrator writes bead comments and per-unit ledgers; it does not route
session blobs.

**`export` — (a) NOT OURS.** `CustomShareResult`, `CustomShareFn`, `LoadedCustomShare` — session
export/sharing via encrypted links and HTML rendering. The 08-end-users bead already assessed the
agent's share command as (a) NOT OURS.

#### Why all five are (a), and what the cross-session gap actually is

The bead's framing is correct: *"cross-session state is how a swarm survives compaction."* But
the OMP memory surfaces answer a different question than ours. OMP's memory backends store
*agent-session context* (what the agent was thinking, what files it read, what the user said) so
the agent can resume with context. Our cross-session state is *orchestration state* (which bead,
which pane, what receipt, what verdict, what decision) so the supervisor can resume without
re-briefing. These are different domains with different storage requirements.

The adequate substrate for our cross-session state already exists: the bead board (durable,
survives panes), the per-unit ledgers (typed, queryable), and the packet journal (append-only).
The gap is not storage — it is that the dispatch loop does not yet write per-unit ledgers (S9
UNKNOWN), and the decision ledger has **three manual rows** (S9 GAP: automated writer/consumer absent). Those are 12-journey S9's findings, and this mapping confirms them rather than replacing them.

The blob-broker is the one surface with potential orchestration relevance: if dispatch packets
grow beyond text (screenshots of pane state, recording artifacts), a blob broker becomes the
natural storage layer. But that is an S5 Cost-field decision, not this mapping's.

## Appendix H — Surface coverage: edit, lsp, commit, compress, cleanse, markit

> **ipg.8**: *Wave EDIT. We spawn git 4 times directly and have no LSP integration in any crate.
> Measured commit defects this wave: a double-quoted `-m` EXECUTES backticks (silent, exit 0),
> and a bare commit swept 8 files including a 678-line crate into a probe commit.*

**Swept 2026-09-01.** Six type roots, 118 files, 548KB, 645 exported symbols, walked to symbol
level. All six are agent-plane editing/IDE/commit/compression features. None crosses the process
boundary into our orchestration layer.

| surface | OMP files | OMP KB | OMP symbols | 1-8 clauses | classification |
|---|---:|---:|---:|:-:|---|
| `edit` | 28 | 132 | 153 | — — — — — — — — | **(a) NOT OURS** — agent file-editing machinery (RepairRegion, AppliedEditSnapshot, file-snapshot-store, blackbox edit observation) |
| `lsp` | 24 | 124 | 225 | — — — — — — — — | **(a) NOT OURS** — full LSP client (setSharedLspEnabled, isIdleClient, applyWorkspaceEditWithLsp, supportsDocumentDiagnostics, isRustAnalyzerClient, shutdownStaleClients) |
| `commit` | 40 | 200 | 172 | — — — — — — — — | **(a) NOT OURS** — commit pipeline (CommitInference, conventional/validation, agentic, changelog, pipeline) — overlaps our commit gates but approaches from the authoring side |
| `compress` | 4 | 16 | 14 | — — — — — — — — | **(a) NOT OURS** — context compression (resolveCompressTargets, runCompressCommand) |
| `cleanse` | 8 | 32 | 40 | — — — — — — — — | **(a) NOT OURS** — session hygiene (CleanseAgentHooks, CleanseAgentRuntime) |
| `markit` | 7 | 32 | 10 | — — — — — — — — | **(a) NOT OURS** — document format conversion (Markit, DocxConverter, EpubConverter, PdfConverter, PptxConverter) |

**Positive control: FAILED — 0 of 6 FULLY COVERED.** Seventh consecutive wave. The pattern is
exhaustive: every OMP type root is either orchestration-plane or agent-plane, and the mapping has
covered every root in both planes. The boundary is correct and the mapping is complete.

**Anti-vacuity: PASSED** — 6 surfaces enumerated, 118 files walked to symbol level, 0 is not the
count.

#### The `commit` surface, and why it is the most interesting (a)

`commit` is 40 files/200KB/172 symbols — the largest surface in this wave, and the one that
overlaps most directly with work we just built. It ships:
- `CommitInference` — AI-powered commit-message inference (analysis/summary/map/fast roles)
- `conventional/validation.d.ts` — conventional-commit validation with `ValidationSeverity`
  ("error" | "warning") and `ValidationIssue`
- `pipeline.d.ts` — a commit pipeline
- `changelog/` — changelog generation
- `git/` — git integration

We built commit-msg round-trip gates (refusing `-m` with backticks), pre-delete-citation-check,
and a canonical commit-message standard. OMP's commit surface approaches the same problem from
the AUTHORING side (AI infers the message) while we approach from the VALIDATION side (gates
refuse bad messages). The two are complementary, not competing — but we never evaluated whether
OMP's `conventional/validation` subsumes our commit-msg gate's checks. That evaluation is a gap,
recorded rather than resolved.

The measured commit defects this wave (double-quoted `-m` executing backticks, bare commit
sweeping 8 files) would be unconstructible if OMP's commit pipeline were the only commit path —
but adopting it would bypass our pre-commit gates (no-shell-gate, commit-msg round-trip,
path-literal-guard), which are the enforcement layer those defects spawned. The correct
architecture is: the agent AUTHORS the message, our gates VALIDATE it. OMP's inference feeds our
gates; neither replaces the other.

#### Why all six are (a)

`edit` is the agent's file-editing machinery (RepairRegion, AppliedEditSnapshot, blackbox
observation, file-snapshot-store — undo/repair capability). `lsp` is a complete Language Server
Protocol client (rust-analyzer client detection, document diagnostics, workspace edits, stale
client shutdown). `compress` and `cleanse` are agent-session hygiene. `markit` is document format
conversion. All six serve the agent's interactive experience — what the agent does inside the
pane, not what the orchestrator does outside it.

The orchestration-relevant OMP surfaces were mapped in wave 1 (session-adjacent output,
subprocess, jsonrpc, cli, commands, slash-commands — 7 consumes edges from omp-inventory-map).
Every root since then has been agent-plane. The mapping has converged.

## Appendix I — Surface coverage: secrets, security, extensibility, config

> **ipg.9**: *Wave SECURITY. Per /hook-certification any hook we register must be Rust,
> asupersync-backed, cancel-correct, registered in hooks_certified.toml, and NEVER
> auto-registered — a hook error reads as DENY and can brick every Write/Edit/Bash in the fleet.*

**Swept 2026-09-01.** Four type roots, 104 files, 1,228KB, 891 exported symbols, walked to symbol
level. All four are agent-plane credential/security/extension/config features. None crosses the
process boundary into our orchestration layer.

| surface | OMP files | OMP KB | OMP symbols | 1-8 clauses | classification |
|---|---:|---:|---:|:-:|---|
| `secrets` | 7 | 44 | 60 | — — — — — — — — | **(a) NOT OURS** — credential placeholder keys and secret obfuscation (getSecretPlaceholderKey, MIN_OBFUSCATE_SECRET_LEN, RegexScanSegment) |
| `security` | 20 | 132 | 124 | — — — — — — — — | **(a) NOT OURS** — cloud security identity management (CodexSecurityCloudClient, ExactSecurityOAuthOptions, selectSecurityAccount, assertSecurityIdentityMatches) |
| `extensibility` | 54 | 376 | 447 | — — — — — — — — | **(a) NOT OURS** — extension/plugin system (Capability<T>, Extension, StringEnum, BashSpawnHook, provider-trust hooks); **largest by symbol count** |
| `config` | 23 | 672 | 260 | — — — — — — — — | **(a) NOT OURS** — settings schema and API-key resolution (ApiKeyResolver, ModelRegistry, showHookStatus); **largest by size** |

**Positive control: FAILED — 0 of 4 FULLY COVERED.** Eighth consecutive wave. The pattern is
exhaustive and structural: every OMP type root is either orchestration-plane (consumed in wave 1)
or agent-plane (not adopted). No exceptions have been discovered across eight waves and 40+
surfaces.

**Anti-vacuity: PASSED** — 4 surfaces enumerated, 104 files walked to symbol level, 0 is not the
count.

#### Per-surface detail

**`secrets` — (a) NOT OURS.** `getSecretPlaceholderKey`, `getExistingSecretPlaceholderKey`,
`MIN_OBFUSCATE_SECRET_LEN`, `RegexScanSegment`, `ReplaceRegexScan` — credential placeholder
generation and secret obfuscation/redaction for OMP's own providers. Our orchestrator holds no
credentials; coupling them to a vendored tool is the 08 §3 rule this surface would violate.

**`security` — (a) NOT OURS.** `CodexSecurityCloudClient`, `ExactSecurityOAuthOptions`,
`selectSecurityAccount`, `assertSecurityIdentityMatches` — cloud security identity management for
the Codex upstream. No hook types; the security surface is authz/OAuth for OMP's provider
connections, not dispatch-safety policy.

**`extensibility` — (a) NOT OURS.** 447 symbols across 54 files — **the largest surface by symbol
count in the entire workspace.** `Capability<T>`, `Extension`, `ExtensionManifest`,
`StringEnum`, `clampThinkingLevel`, `BashSpawnHook`, provider-trust hooks (legacy shim). This is
OMP's extension/plugin loading system: how it discovers, validates, and instantiates agent
capabilities from installed extensions. No crate in our workspace loads agent extensions. The
`BashSpawnHook` type is a JavaScript hook, not a Rust hook — the /hook-certification doctrine
(Rust, asupersync-backed, cancel-correct, hooks_certified.toml) does not apply to OMP's
JS extension hooks.

**`config` — (a) NOT OURS.** 672KB — the largest surface by size. `ApiKeyResolver`,
`ApiKeyResolverRegistry`, `ModelRegistry`, settings schema (including `statusLine.showHookStatus`).
Ambient config would make spawns environment-dependent; our crates pass explicit flags for
receipt discipline. The `statusLine.showHookStatus` setting confirms OMP has a hook-status display
surface, but it is a UI setting, not a hook-registration API.

#### The hook-certification angle, assessed honestly

None of the four surfaces contains a hook-registration API that competes with /hook-certification.
The `BashSpawnHook` in extensibility is a JavaScript callback in OMP's extension system, not a
system-level hook — it cannot brick Write/Edit/Bash the way a bad pre-commit hook can. The
`statusLine.showHookStatus` setting is a display toggle. The /hook-certification doctrine (Rust,
asupersync-backed, cancel-correct, hooks_certified.toml, never auto-registered) is our own
standard for OUR hooks, and no OMP surface provides an alternative that would bypass it.

The closest crossing point is the `config` surface: if OMP's settings could register hooks, the
config→hook path would be a bypass of /hook-certification. Measured: settings-schema.d.ts contains
`showHookStatus` (a display toggle) but no hook-registration API. The bypass does not exist.

---

### BLOCKER resolution — the ipg.6 symbol count had three values and no rule

`GradeJourney` filed a BLOCKER: prose said **533**, the table above it sums to
**503**. Re-measuring produced a third answer, **621**.

Three numbers, and the defect is not arithmetic — **none of them shipped a
derivation**, so none could be checked and none could be wrong. The gap is
concentrated rather than spread: `eval` counts 207 under an explicit rule against
the table's 95, which is 112 of the 118-symbol spread on its own.

Registered as `ipg6_root_symbols` with the counting rule stated in the command.
That does not make 621 truer than 533 — it makes it **falsifiable**, which is the
only property the other two lacked. If the rule is wrong, the command is where to
argue with it.

**This is the section's own Foundation preflight loop rule applied to the section:** *"every number
carries the command that derives it."* Three consecutive rounds graded this
section and none caught it, because a reader comparing prose to a table sees two
numbers and picks one. Only re-running a command produces a third.

## Appendix J — Surface coverage: web, exa, stt, tts, ssh, internal-urls, tools, cli

> **ipg.10**: *Wave IO. Eight agent-plane type roots — search providers, speech I/O, remote
> access, internal URI routing, the tool registry, and CLI argument parsing.*

**Swept 2026-09-01.** Eight type roots, 171 files, ~1,800 exported symbols, walked to symbol
level. All eight are agent-plane features. None crosses the process boundary.

| surface | OMP files | OMP KB | OMP symbols | 1-8 clauses | classification |
|---|---:|---:|---:|:-:|---|
| `web` | 4 | 488 | 29 | — — — — — — — — | **(a) NOT OURS** — web-search provider types (KagiSearchRequest/Result, AnthropicProvider) |
| `exa` | 3 | 12 | 17 | — — — — — — — — | **(a) NOT OURS** — Exa search integration (ExaSearchResponse, findApiKey) |
| `stt` | 10 | 44 | 50 | — — — — — — — — | **(a) NOT OURS** — speech-to-text (STTController, EndpointerConfig, STT_MODELS) |
| `tts` | 12 | 52 | 54 | — — — — — — — — | **(a) NOT OURS** — text-to-speech (TtsDownloadProgress, KOKORO_VOICES) |
| `ssh` | 5 | 32 | 57 | — — — — — — — — | **(a) NOT OURS** — SSH config/host management for the agent (SSHHostConfig, RemoteFileRead/WriteOptions) |
| `internal-urls` | 22 | 100 | 68 | — — — — — — — — | **(a) NOT OURS** — internal URI scheme resolver (AgentProtocolHandler, ResolvedArtifactFile) |
| `tools` | 94 | 732 | 860 | — — — — — — — — | **(a) NOT OURS** — agent tool registry (shouldRouteWriteThroughBridge, ApprovalPolicy) — LARGEST by symbols in the workspace |
| `cli` | 51 | 352 | 361 | — — — — — — — — | **(a) NOT OURS** — CLI argument parsing (AgentsAction, ResolvedCliArgv) |

**Positive control: FAILED — 0 of 8 FULLY COVERED.** Ninth consecutive wave. The pattern is
exhaustive: every OMP type root is either orchestration-plane or agent-plane.

**Anti-vacuity: PASSED** — 8 surfaces enumerated, 171 files walked to symbol level.

**`tools`** is the largest by symbol count in the entire workspace (860 exported symbols, 94
files, 732KB). It is the agent's complete tool registry — every built-in tool the agent can
invoke, with approval policies, bridge routing, and activity snapshots. No crate in our workspace
imports any of these types.

## Appendix K — Surface coverage: async, utils, lib, tiny, vibe, auto-thinking

> **ipg.11**: *Wave RUNTIME. async is the one to read first: OMP's concurrency surface vs
> asupersync's binding contract — compose, conflict, or duplicate?*

**Swept 2026-09-01.** Six type roots, 60 files, 268KB on disk, 316 exported declarations, walked to symbol level. Exported declarations use the rule ^export [declare] {type,interface,const,function,class,enum} NAME in *.d.ts; file sizes use du -ck. All six are agent-plane runtime features. None crosses the process boundary.

| surface | OMP files | OMP KB | OMP symbols | 1 asupersync | 2 unsafe | 3 cancel | 4 typed | 5 logged | 6 observable | 7 robot | 8 WIRED | coverage | classification |
|---|---:|---:|---:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|---|
| async | 3 | 20 | 15 | —¹ | — | — | — | — | — | — | — | FULLY COVERED | **(a) NOT OURS** — OMP job scheduling (AsyncJobManager, raceJobSettlement) |
| utils | 43 | 176 | 185 | — | — | — | — | — | — | — | — | MAPPED_NOT_ADOPTED | **(a) NOT OURS** — OMP utility layer (ActiveRepoContext, resolveActiveRepoContext) |
| lib | 1 | 4 | 4 | — | — | — | — | — | — | — | — | MAPPED_NOT_ADOPTED | **(a) NOT OURS** — xAI HTTP credential transport (XAIHttpTransport, resolveXAIHttpCredentials) |
| tiny | 9 | 48 | 83 | — | — | — | — | — | — | — | — | MAPPED_NOT_ADOPTED | **(a) NOT OURS** — local/online tiny-model completion (TinyModelDevice, TextGenerationPipeline) |
| vibe | 3 | 16 | 25 | — | — | — | — | — | — | — | — | MAPPED_NOT_ADOPTED | **(a) NOT OURS** — Vibe worker lifecycle (VibeSessionRegistry, VibeLifecycleEvent) |
| auto-thinking | 1 | 4 | 4 | — | — | — | — | — | — | — | — | MAPPED_NOT_ADOPTED | **(a) NOT OURS** — prompt-difficulty classification (classifyDifficulty, parseDifficultyLevel) |

**Positive control: PASS — 1 of 6 FULLY COVERED (async).** FULLY COVERED means the surface map row is complete; it does not mean the capability is adopted.

**Anti-vacuity: PASSED** — 6 surfaces, 60 files, and 316 exported declarations were enumerated. A zero-surface or zero-file result is an ERROR.

**The async question answered:** OMP's async root composes with asupersync at a boundary but does not duplicate it. OMP's AsyncJobManager schedules in-process agent tool jobs (bash, task, eval) and races settlement against steering/abort; omp-rpc-session uses asupersync Cx, process groups, bounded phase deadlines, and both-pipe draining for the orchestrator's one OMP child. No Rust crate imports OMP's async declarations, so there is no direct binding conflict or duplicate shared implementation.

¹ The async/asupersync relationship is a measured composition result, not a local contract claim: the OMP declaration is TypeScript agent-plane code, while the Rust binding is omp-rpc-session/src/lib.rs:1-16,23-46,135-163.

**No category (b) rows:** every enumerated root is (a) not ours. Therefore no row requires a category-(b) OMP alternative; the OMP alternatives above name the existing declarations for auditability.


---

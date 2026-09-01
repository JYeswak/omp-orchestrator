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
| **OMP** | Oh My Pi, the agent CLI this orchestrator wraps; current host recheck: `omp/18.1.2` (the retained scanner snapshots target `omp/18.0.11`) |
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
> **Upstream type for the receipts gap:** `IrcDeliveryReceipt` + `AsyncJobDeliverySink` (`tools/hub/types.d.ts:8,84`) already ship. The demand above is DECLARED ONLY — no wire path measured — not precedent-free, and this row must not be read as "nothing exists upstream".
> **Completion is not precedent-free.** `AgentEndEvent.willContinue` + `SessionStopEvent` ship on `RpcSessionEventFrame` and were observed crossing the wire. Any "precedent-free" language in this table is about OUR consumption, never about the platform's vocabulary.

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
| `omp` | `omp/18.1.2` (host recheck; scanner snapshots remain `omp/18.0.11`) | `/Users/josh/.local/bin/omp` |
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
> **Upstream type for the idle gap:** `GuestIdleReconcilerCtx` (`dist/types/collab/guest.d.ts:9-30`) declares the NewlyIdle/ConfirmedIdle distinction this layer re-derives by hand. DECLARED ONLY: the type exists, no local consumer reads it.
> **Receipts:** `IrcDeliveryReceipt` (`tools/hub/types.d.ts:8`) exists upstream. `cp-z42vu` names a defect in what WE consume — a send reporting success while the packet never landed — not an absent upstream type.

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
| Q6 | What happens when OMP changes under us? | **OPEN** — the host now reports `omp/18.1.2`; the retained scanner snapshots target `omp/18.0.11`, and no compatibility policy governs upgrades. The pre-extraction scanner's 799-versus-136 slash-command mismatch remains historical. | orchestrator |
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

# AGENTS.md — omp-orchestrator
> **SUMMARY:** This repository is the Rust extraction of the OMP orchestrator: typed ground truth,
> readiness, selection, dispatch, verification, and reaping. Read this file completely before acting;
> the lifecycle, cancellation, gate, and evidence rules below are binding.
**Session start:** read this file, `CLAUDE.md`, and `NEGATIVE_EVIDENCE.md` before acting; after compaction, re-read them before resuming.
The operating manual for any agent working this repo. `README.md` says what the product is and why.
This file says how you work here, what every crate is for, and what "done" means.

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

## `cargo test` refuses to run? Use the bypass. (measured 2026-09-04)

**Read this before reporting a test as unrunnable.** A bare `cargo` invocation in this repo goes
through an `rch` wrapper that offloads to remote workers and **refuses local fallback**:

```
[RCH] remote required; refusing local fallback
      (no admissible workers: insufficient_total_slots=3,active_project_exclusion=1) — retryable
```

**The bypass runs it locally and works:**

```bash
RCH_CARGO_WRAPPER_BYPASS=1 cargo test -p <crate>
```

**MEASURED COST OF NOT DOCUMENTING THIS: a pane declined a MUTATION-VERIFIED claim it could have
earned.** On 2026-09-04 a worker landed `ae115ec` and reported *"cargo test blocked: rch hook
refuses local fallback (no admissible workers). Not MUTATION-VERIFIED."* That refusal was correct
discipline — an unrunnable test is not a passing test. But the bypass had been in use by the
conductor all through the previous session, and `grep -c RCH_CARGO_WRAPPER_BYPASS` returned **0 in
`AGENTS.md`, 0 in `CONTRACT.md`, 0 in `NEGATIVE_EVIDENCE.md`** — every place a pane would look. The
knowledge existed only in the conductor's shell history, which is the *dispatch-only instruction*
failure aimed at a tool rather than a requirement.

**Two things the bypass does NOT license.**

1. **It changes WHERE the build runs, never WHAT the verdict means.** `NE-001` records three
   offloaded `101`s in one day whose causes were SIGKILL, SIGKILL, and a tracked file the worker
   never received — **zero compile errors** between them. An offloaded exit code is not a verdict
   about the local target. Read `signal:` before `E`.
2. **It is also how you get the wrong artifact.** A bare `cargo build` was silently offloaded and
   returned an `x86-64 ELF` on this `arm64 Darwin` host; five ledger writes failed with
   `cannot execute binary file` and landed nothing. If a freshly built binary will not run,
   `file <binary>` first, then rebuild with the bypass. This is `HD-0013`'s rule in practice:
   **contabo for Linux, the local Mac for darwin.**

**NO-CLAIM.** The bypass makes the local run possible; it does not make the remote lane healthy.
`insufficient_total_slots=3` and `active_project_exclusion=1` are unexplained here, and the wrapper
calls the refusal *retryable*, so a worker fleet problem remains UNMEASURED rather than fixed.

---

## The zero-worktree policy (binding, Joshua 2026-09-03)

**All work happens on `main`, in this one checkout, saved to `main`.** No worktrees. No branches.
Concurrency is managed by **file reservations**, not by giving each agent its own copy of the tree.

> *"we have a zero worktree policy — all work MUST happen on main — no exceptions. worktrees and
> branches cause shit to not get saved."* — Joshua, 2026-09-03

**The one exception, stated exactly:** a **test** may create a worktree **if it deletes it**. The
worktree must not outlive the test that made it. Nothing else may create one — not a build, not a
lane, not an agent wanting a clean tree, not a "temporary" experiment.

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

- **(b) reimplemented by scraping — 4:** pane state, dispatch, session, health check. Each has an OMP
  RPC or CLI alternative *that exists today*. These are not gaps; they are rewrites of shipped
  surface, done through a terminal.
- **(c) should use — 5:** `omp/muxConnect`, `omp/muxPing`, `omp/muxRestartServer`, `goals`, `collab`.
  Nothing in `crates/` mentions any of the five.

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

## OMP lifecycles — what they are and where to find them

OMP (Oh My Pi) v18.0.11 — node CLI "@oh-my-pi/pi-coding-agent", repo "can1357/oh-my-pi". 29 built-in
tools plus 3 hidden (yield, goal, think), 136 slash commands, and **39 CLI subcommands** — counted,
not estimated, from the COMMANDS block of `omp --help`:

~~~bash
omp --help | awk '/^COMMANDS/{f=1;next} f&&/^[[:space:]]*$/{exit} f&&/^  [a-z]/{c++} END{print c}'
~~~

Measured output: `39`.

The installed RPC handler exposes **42 inbound JSON-RPC command methods**; the derivation command and
its output are below. Static production reachability in the **control-plane** adapter — a *different*
repo — is **5/42**. In **this** repo it is **0/42**, and that zero is the fifth rule.

**Retired figure: "81 JSON-RPC methods, and we currently use 17 of the 81."** That pair was
**inherited**, ships **no command that produces it**, and **could not be re-derived** on 2026-08-31
against the installed binary. The reproducible figures are the **42** handler methods below, the
**39** subcommands above, **3** `omp/*`-prefixed methods in the bundle (`omp/muxConnect`,
`omp/muxPing`, `omp/muxRestartServer`), and **57 directories + 14 declaration files** under
`dist/types`. **81 and 17 are retired — cite neither.** And do not read their retirement as a
*smaller* surface: what is measurable is larger than 81 and we consume none of it.
`omp-orchestrator-omp-surface-map-41b` owns producing the real per-crate table.

### Installed RPC command census (measured 2026-08-31)

Version gate and source identity:

  omp --version -> omp/18.0.11
  /Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js
  SHA-256: a95635ad43ab85fcabcbee9bbcc593d9ea8e68ba54228b4c9fdbd1e25766281c; bytes: 19803745.

This command derives the method list from the installed binary's RPC dispatch handler; it is not a
hand-transcribed table:

~~~bash
omp --version && bun -e 'const p="/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js"; const s=await Bun.file(p).text(); const start=s.indexOf("let w=async(v)=>"); const end=s.indexOf("},E=new KWt",start); const methods=[...s.slice(start,end).matchAll(/case"([^"]+)"/g)].map(x=>x[1]); console.log("RPC_COMMAND_METHODS="+methods.length); console.log(methods.join("\n"));'
~~~

Measured output: RPC_COMMAND_METHODS=42.

negotiate_protocol, prompt, steer, follow_up, abort, abort_and_prompt, new_session, switch_session,
branch, get_state, set_fast_mode, get_available_commands, set_todos, set_host_tools,
set_host_uri_schemes, set_subagent_subscription, get_subagents, get_subagent_messages, set_model,
cycle_model, get_available_models, set_thinking_level, cycle_thinking_level, set_steering_mode,
set_follow_up_mode, set_interrupt_mode, compact, set_auto_compaction, set_auto_retry, abort_retry,
bash, abort_bash, get_session_stats, export_html, get_branch_messages, get_last_assistant_text,
set_session_name, handoff, get_messages, get_messages_page, get_login_providers, login.

### Static production reachability (measured 2026-08-31)

Scope: production Rust under /Users/josh/Developer/control-plane/crates/xtask/src/; tests, comments,
and compatibility tables are excluded. This command derives Rust constructor call sites and maps each
constructor through RpcRequest::to_frame to the installed handler method:

~~~bash
bun -e '
const installedPath="/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js";
const installed=await Bun.file(installedPath).text();
const handlerStart=installed.indexOf("let w=async(v)=>");
const handlerEnd=installed.indexOf("},E=new KWt",handlerStart);
const installedMethods=[...installed.slice(handlerStart,handlerEnd).matchAll(/case"([^"]+)"/g)].map(m=>m[1]);
const sourcePath="/Users/josh/Developer/control-plane/crates/xtask/src/omp_rpc.rs";
const source=await Bun.file(sourcePath).text();
const frameStart=source.indexOf("pub fn to_frame");
const frameEnd=source.indexOf("pub fn handshake_requests",frameStart);
const frameSource=source.slice(frameStart,frameEnd);
const variantToMethod=new Map();
for(const match of frameSource.matchAll(/Self::([A-Za-z]+)(?:(?!Self::)[\s\S]){0,800}?"type"\s*:\s*"([^"]+)"/g)) variantToMethod.set(match[1],match[2]);
const start=source.indexOf("pub fn handshake_requests");
const end=source.indexOf("\n}",start);
const rows=[];
for(let lineStart=start;lineStart<end;){const lineEnd=source.indexOf("\n",lineStart);const stop=lineEnd<0||lineEnd>end?end:lineEnd;const match=source.slice(lineStart,stop).match(/RpcRequest::([A-Za-z]+)/);if(match){const method=variantToMethod.get(match[1]);if(!method||!installedMethods.includes(method))throw Error("unmapped RPC constructor: "+match[1]);rows.push(sourcePath+":"+source.slice(0,lineStart).split("\n").length+" "+method)}lineStart=stop+1}
const unique=[...new Set(rows.map(row=>row.slice(row.lastIndexOf(" ")+1)))];
console.log("installed_rpc_commands="+installedMethods.length);
console.log("static_production_rpc_commands="+unique.length+"/"+installedMethods.length);
console.log(rows.join("\n"));
'
~~~

Measured output:

installed_rpc_commands=42
static_production_rpc_commands=5/42
/Users/josh/Developer/control-plane/crates/xtask/src/omp_rpc.rs:275 negotiate_protocol
/Users/josh/Developer/control-plane/crates/xtask/src/omp_rpc.rs:276 get_state
/Users/josh/Developer/control-plane/crates/xtask/src/omp_rpc.rs:277 get_available_commands
/Users/josh/Developer/control-plane/crates/xtask/src/omp_rpc.rs:278 get_available_models
/Users/josh/Developer/control-plane/crates/xtask/src/omp_rpc.rs:279 set_fast_mode

RpcRequest::CancelUiRequest at omp_rpc.rs:740 emits the separate extension_ui_response frame and is
intentionally excluded from the inbound RpcCommand denominator.

This is **static reachability**, not runtime usage. It proves production constructors exist in the scanned
adapter source; it does not prove a live OMP process, provider response, or invocation through an
unscanned path.

### The RPC lifecycle (typed, in crates/xtask/src/omp_rpc.rs in control-plane)

Read the enum, not this table, when precision matters — this is a map to the source.

| State | Meaning |
|---|---|
| `Spawned` | Child started; no `ready` yet |
| `Ready` | `ready` observed **and** it advertised the required version |
| `Negotiated` | `negotiate_protocol` v2 answered successfully |
| `Active` | Handshake complete: every issued request answered, metadata observed |
| `Stopping` | Input closed; awaiting exit |
| `Stopped` | Clean terminal |
| `Failed` | **Restrictive** terminal — see `FailureKind` |
| `TimedOut` | **Restrictive** terminal — a bounded wait elapsed |

Two properties carry the weight:

- **Terminal states admit no further transition.** The machine, not the caller, enforces it.
- **A restrictive terminal is one a caller must not read as success.** `Failed` and `TimedOut` are
  restrictive. This is why *a timeout is not a verdict*: an empty buffer from a killed child must
  map to `TimedOut`, never to the token a genuinely failing subject produces.
- **No wait in the adapter is unbounded, including shutdown.**

Supporting types: `LifecycleMachine` (transitions), `LifecycleReport` (the observable outcome of one
run), `TimeoutPhase` (which bounded wait elapsed), `FailureKind` (why a restrictive terminal).

### The pane lifecycle (what an operator sees)

Distinct from the RPC lifecycle and more often wrong, because it is read from a terminal.

**The v18 status-line contract, measured 2026-08-31:**

- **Working** — a braille spinner followed by an **elapsed timer** (`⠸ 4m`)
- **Idle** — the `π` prompt glyph where the spinner would be

The shipped NTM presets required the literal word `Working`, which v18 **never renders**. The
classifier scored **0/3 on live payload** at 03:08Z and **3/3** after the fix (`d05200c`).

**Read the LAST status line, never the buffer.** A whole-buffer scan matches a stale spinner still
in scrollback: one pane scored *working AND idle simultaneously* while genuinely idle.

**Two captures or it is not a claim.** `Working (27s)` and a frozen pane render identically. Compare
timer **and** spinner-stripped content hash ≥75s apart.

**`safe_to_dispatch` is not liveness.** A wedged pane accepts a packet, parks it at
`Press up to edit queued messages`, and never submits it.

### The bead lifecycle (the unit of work)

`open → in_progress (claimed) → closed (with cited evidence)`, with two traps that are *ours*, both
measured:

1. **The close reason must start with** `MUTATION-VERIFIED` / `DONE` / `APPROVED` / `WONTFIX`.
   A prose reason is refused by policy, the refusal scrolls past, and the agent believes it landed.
2. **A child blocked by its parent epic cannot close.** An epic closes *after* its children, so that
   dependency is inverted and makes both permanently unclosable. `--force` with the reason recorded
   is correct when the epic is the only blocker.

---

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

## The crate extraction target list — what each one is, and **which repository it is actually in**

**Read the STATUS column before you reason about any row.** The table below has 24 rows and is a
**historical extraction-target list, not the package inventory.** The workspace is now far larger
than the table; a row marked CONTROL-PLANE is not an available local dependency and must not be
cited as present.

### DO NOT CITE A PACKAGE COUNT FROM THIS FILE. RUN THE COMMAND.

**This figure has moved `27 → 50 → 51 → 65` inside the lifetime of this one document**, and each
stale value was corrected by a later agent who then wrote a fresh integer that went stale in turn.
A count in prose is wrong the moment anyone lands a crate, and this section has now proven that
four times. The correction is not a better number — it is **no number**:

```bash
# in either repo; these two must agree, or you have a manifest-vs-directory discrepancy
cargo metadata --no-deps --format-version 1 --offline \
  | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["packages"]))'
find crates -mindepth 1 -maxdepth 1 -type d | wc -l

# names present in BOTH repos
comm -12 <(find crates -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort) \
         <(find /Users/josh/Developer/control-plane/crates -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort)
```

**One dated measurement, as evidence that the commands run — never as a figure to cite.** Measured
2026-09-02 by the orchestrator: **65 here** (`cargo metadata` and the directory count agree),
**62 in control-plane**, **28 names in both**. Compare against the previously-published `27 / 59 / 4`:
every one of the three moved, and the intersection grew **7×**. The extraction wave is landing
crates faster than any prose table can track it.

**There is no percent-ported figure, and there cannot be one from this file.** The numerator moves
hourly and the denominator was never established — the extraction scope has been asserted as 20 and
as 23 crates and **neither figure ever shipped a producing command.** Treat both the way the retired
"81 JSON-RPC methods, 17 used" pair is treated: cite neither. `NUMBERS.toml` exists precisely so
plan sections resolve figures to a runner instead of typing integers; a section that types an
integer here is a defect, not a shortcut.

### Current workspace packages outside the legacy extraction table

These are HERE, but were not part of the 24-row extraction target list:

| Crate | STATUS |
|---|---|
| ack-spine | **HERE** |
| ack-stage | **HERE** |
| commit-build-fence | **HERE** |
| dispatch-claim-fence | **HERE** |
| dispatch-silence-watch | **HERE** |
| finding | **HERE** |
| finding-dispatch | **HERE** |
| installer | **HERE** |
| kernel-bypass-gate | **HERE** |
| kernel-only-operator-hook | **HERE** |
| no-shell-gate | **HERE** |
| omp-inventory-map | **HERE** |
| omp-orchestrator | **HERE** |
| omp-rpc-session | **HERE** |
| omp-types | **HERE** |
| path-literal-guard | **HERE** |
| porting-gate | **HERE** |
| pre-delete-citation-check | **HERE** |
| receiver-receipt | **HERE** |
| state-wildcard-lint | **HERE** |
| subprocess-contract | **HERE** |
| tick-monitor | **HERE** |
| undrained-pipe-lint | **HERE** |
LOC and `tests/` counts on every `CONTROL-PLANE` row are read from the control-plane working tree.
They describe source you do not have here. Grouped by the lifecycle stage they serve.

### Ground truth — "what is actually true right now"

These exist because **every classifier we trusted has been wrong at least once**, and a wrong
liveness read either interrupts real work or leaves a worker idle beside a full queue.

| Crate | STATUS | LOC | What it does | Why it exists |
|---|---|---:|---|---|
| `pane-truth` | **CONTROL-PLANE** | 1247 | Ground-truth tmux pane state | The shell version remains the differential oracle; this is the typed reading |
| `fleet-truth` | **CONTROL-PLANE** | 1621 | Fleet-wide inspection register | One place answers "what is the fleet doing" so callers stop re-deriving it |
| `fleet-reconcile` | **CONTROL-PLANE** | 1424 | NTM projection vs tmux reality | NTM's snapshot returns `total_sessions: 0` with `success: true` when stale; tmux does not lie |
| `oracle-compare` | **CONTROL-PLANE** | 449 | Shared comparator: claim vs independent oracle | An empty or unreadable oracle must be an ERROR, never a silent agreement |
| `pane-oracle-diff` | **CONTROL-PLANE** | 741 | tmux pane census vs ntm projection | Catches projection drift before a dispatch rides it |
| `oracle-pane-state-differential` | **CONTROL-PLANE** | 613 | session:index pane-set differential (tmux vs ntm) | Uses the shared set comparator; this source has no Z3 implementation |
| `fleet-composite` | `HERE` | 1372 | Geometric fleet-health composite and diagnostic CLI | Refuses malformed, empty, and non-finite inputs instead of inventing a score |

### Readiness and admission — "may this pane receive work"

| Crate | STATUS | LOC | What it does | Why it exists |
|---|---|---:|---|---|
| `pane-dispatch-ready` | **CONTROL-PLANE** | 1555 | Can this pane SAFELY receive a dispatch | `safe_to_dispatch` is not liveness |
| `pane-dispatch-fence` | `HERE` | 468 | Cross-process per-pane admission fence | Two dispatchers landing during a `/clear` vaporise the packet |
| `composer-typed` | `HERE` | 556 | Does the composer hold real TYPED text | Sender success is not receiver receipt |
| `ntm-fleet-monitor` | **CONTROL-PLANE** | 3122 | Typed fleet actions + approval waves. **Classifies; does not send** | Separating classification from actuation makes the verdict auditable |

### Selection — "what should be worked next"

| Crate | STATUS | LOC | What it does | Why it exists |
|---|---|---:|---|---|
| `loop-queue-filter` | `HERE` | 912 | Fail-closed queue selector | Epics invite unbounded scope; in-flight work must not be re-offered |
| `loop-coverage` | **CONTROL-PLANE** | 926 | Typed coverage matrix. **A map, not a gate** | Says honestly what is *not* covered rather than implying completeness |
| `refill-idle-panes` | **CONTROL-PLANE** | 842 | Refill every idle pane from the bv DAG | An idle worker beside a ready queue is the conductor's failure |
| `omp-idle-dispatch` | **CONTROL-PLANE** | 1667 | Fail-closed idle OMP pane dispatch lane | Makes repository, session, ledger, and admission inputs explicit before dispatch |

### Dispatch — "send the work"

| Crate | STATUS | LOC | What it does | Why it exists |
|---|---|---:|---|---|
| `fast-dispatch` | **CONTROL-PLANE** | 2292 | Admit on a fresh standing verdict, select free panes | Must fail closed on a stale verdict |
| `tick-dispatch` | **CONTROL-PLANE** | 990 | Ground-truth pane dispatch fence | Decided by tmux/ntm truth, not a cached label |
| `loop-driver` | **CONTROL-PLANE** | 2484 | Single-instance, deadline-bounded driver | Two ticks fighting over one pane is corruption |
| `loop-tick` | **CONTROL-PLANE** | 1480 | Single-pane dispatch tick | The unit the driver repeats |
| `fleet-monitor` | **CONTROL-PLANE** | 2569 | OBSERVE lane: attention wait + idle/ready scan | Block on a state transition; polling is the anti-pattern |

### Verification and reaping — "did it actually happen"

| Crate | STATUS | LOC | What it does | Why it exists |
|---|---|---:|---|---|
| `verify-dispatch` | **CONTROL-PLANE** | 1291 | Verification from **bead status only** | Ground truth, never a pane's self-report |
| `dispatcher-deadman` | **CONTROL-PLANE** | 883 | Watchdog: eligible work that received no packet | The failure that is invisible because everything looks healthy |
| `reap-finished-panes` | **CONTROL-PLANE** | 1189 | Sweep finished panes before the next dispatch | An unreaped pane is capacity that silently disappears |
| `wired-but-inert-guard` | **CONTROL-PLANE** | 1394 | Fail-closed proof that declared dispatch gates are actually invoked | Prevents a green unused gate from counting as coverage |

**Dependency shape** (from each `Cargo.toml`, current 24-row table): 17 leaves with zero path deps;
7 with exactly one — `ntm-fleet-monitor` → `loop-coverage`, `fleet-monitor` →
`ntm-fleet-monitor`, `pane-oracle-diff` → `oracle-compare`,
`oracle-pane-state-differential` → `oracle-compare`, `tick-dispatch` → `oracle-compare`,
`fast-dispatch` → `loop-switch`, and `loop-driver` → `loop-switch`. **Extract leaves first.**

### Porting order over the whole source workspace (measured 2026-08-31)

The dependency shape above is scoped to **the 24 rows of this table only**. The extraction frontier
is the whole source workspace, and it is larger. Derived from the resolver, not from text:

```bash
# Run in /Users/josh/Developer/control-plane. Topology comes from cargo, never from grep.
/Users/josh/.cargo/bin/cargo metadata --no-deps --format-version 1 \
  | jq -r '[.packages[] | {n: .name,
                           d: ([.dependencies[] | select(.path != null) | .name] | unique | length)}] as $p
           | "members=\($p | length)",
             "leaves=\([$p[] | select(.d == 0)] | length)",
             "one-dep=\([$p[] | select(.d == 1)] | length)",
             "two-plus=\([$p[] | select(.d >= 2)] | length)"'
```

Result: **57 members — 33 true leaves (zero intra-workspace path deps), 23 with exactly one, and 1
with two** (`controller-tick` → `loop-switch`, `admission-reason`). `crates/loop-tick/Cargo.toml`
declares its own `[workspace]` and is therefore **not** one of the 57; measured standalone it is
also a zero-path-dep leaf, so the leaf count is **33 of 57 loaded, or 34 counting the excluded
manifest**. Cite which denominator you mean. **Extract leaves first**: a leaf ports without
dragging a second crate across the repo boundary.

**The topology must not come from grep, and here is the actual reproduction** — corrected, because
the first diagnosis published for this was also wrong, which is the more useful lesson. The
conductor's original loop reported **1 leaf out of 59** where `cargo metadata` reports 33. The
published explanation was "the pattern missed Cargo's inline-table syntax." **That explanation is
false.** The pattern matched fine; only **22 path lines exist across all 57 manifests**, so most
crates genuinely have no match. The real cause is one shell idiom:

```bash
d=$(grep -c 'path = "\.\./' "crates/$c/Cargo.toml" 2>/dev/null || echo 0)
[ "$d" = "0" ] && n=$((n+1))     # never fires
```

`grep -c` **already prints `0`** and *then* exits 1 when nothing matches, so `|| echo 0` appends a
**second** zero. `d` becomes `$'0\n0'`, the equality test fails, and every zero-dependency crate is
scored as *having* dependencies. Measured directly: `d='0'$'\n''0'` → FALSE; dropping the `|| echo 0`
→ `d2=0` → TRUE.

That is the **same family as `[RCH] remote required` exiting 103 with `0 passed 0 failed`**, which I
also briefly read as a test result: *a command's failure path emitting something shaped like data*.
A Rust `count()` returns a `usize` and cannot produce `"0\n0"` — which is the concrete reason this
repo forbids shell rather than merely discouraging it.

A subagent independently **could not reproduce** the claim, because it ran a differently-shaped
command (`grep -rlE`, anchored → 0 files; unanchored → 24). Both of us were measuring real things
and neither was measuring the other's. **A defect report must carry the exact command**, or the
next person disproves a claim you never made. This is the same confident-zero class as the retired
"81 JSON-RPC methods, 17 used" figure above. **Derive topology from `cargo metadata`.**

### Specimen: `pane-truth`, installed here and un-portable to this repo

One row, made concrete, because it is the shape of the whole defect:

- `/Users/josh/.local/bin/pane-truth` — **installed**, 2,489,600 bytes, Aug 31 02:22.
- `/Users/josh/Developer/omp-orchestrator/crates/pane-truth` — **does not exist**.
- Its only source is `/Users/josh/Developer/control-plane/crates/pane-truth`, whose HEAD is
  `407ecb5` — an **unrelated history** to ours, sharing no commit with this repo.

So a binary built from another repository's tree sits on `PATH` under a name this workspace
documents and does not contain. The installer's identity check compares the installed artifact
against **this** repo's HEAD, which it can never equal, and therefore reports **MISMATCH
permanently** — not as a transient staleness signal but as a fixed point. A MISMATCH that can never
clear is not a gate; it is noise that trains operators to ignore the gate. `pane-truth` is not
installed-and-drifted. It is **installed-from-elsewhere**, and no rebuild here changes that until
the crate is actually extracted.

### NO-CLAIM: there is no denominator, so there is no "percent ported"

The 57 control-plane members are **candidates, not a work queue.** Some are cron-lane scaffolding
that should be **deleted rather than moved** — porting them would import a lane we already retired.
Nothing in this file establishes which of the 57 are targets and which are terminal.

The extraction scope has been stated in this repository as **20 crates** and as **23 crates**
(bead `omp-orchestrator-815`), and **neither figure was ever derived from a command.** They were
asserted. With the numerator moving and the denominator never established, **"how much extraction
is left" is undefined**, and any percentage, burndown, or "N of M ported" claim built on these
numbers is unfounded — including one built on the 4-of-24 split above, which measures **this
table**, not the extraction set.

This is the **unstated-denominator defect**, the same failure as the retired
"81 JSON-RPC methods, and we currently use 17 of the 81" pair earlier in this file: an inherited
ratio, no producing command, not re-derivable. That pair is retired and cited by nobody. **Treat
20 and 23 the same way.** The denominator is established by a command that enumerates targets and
names the terminal crates, or it is not established at all.

**Unsafe posture in the current 24-row table: 5 of 24.** `ntm-fleet-monitor`,
`refill-idle-panes`, `omp-idle-dispatch`, `wired-but-inert-guard`, and `fleet-composite`
declare `unsafe_code = "forbid"`. The 815 extraction scope is 23 crates and is also 5-for-23;
the historical 815 comment claiming 3-for-23 is stale after control-plane commit `8fc3e4b`, which
added the lint to the other two ported crates. A crate that will not compile under the lint is a
**finding**, not a reason to drop the lint.

**Measured set reconciliation (2026-08-31).** The pre-audit table had 21 rows, not 20. It included
the real `oracle-pane-state-differential` crate. The three ported crates named by bead
`omp-orchestrator-815` bring the documented table to 24 rows, while 815's stated 23-crate
extraction scope is its original 20 rows plus those three and therefore excludes
`oracle-pane-state-differential`. That is a real scope mismatch, not a rounding issue.

- Target workspace `/Users/josh/Developer/omp-orchestrator`: 8 loaded Cargo packages.
- Source workspace `/Users/josh/Developer/control-plane`: 58 tracked top-level crate manifests;
  Cargo loads 57 packages. The excluded top-level manifest is `crates/loop-tick/Cargo.toml`,
  which declares its own `[workspace]`; the two other tracked manifests are fixture manifests.
- Working-tree source totals for the current 24-row table: 32,087 Rust LOC and 22 crate-level
  `tests/` directories. The 815 23-crate scope totals 31,474 Rust LOC and 21 `tests/`
  directories under the same counting rule.

The audit is re-runnable from the target repo with the source root explicit:

```bash
# Target package count; run in /Users/josh/Developer/omp-orchestrator.
/Users/josh/.cargo/bin/cargo metadata --no-deps --format-version 1 \
  | jq '[.packages[].manifest_path | select(test("/crates/[^/]+/Cargo.toml$"))] | length'

# Source package count; run in /Users/josh/Developer/control-plane. The warnings are meaningful.
/Users/josh/.cargo/bin/cargo metadata --no-deps --format-version 1 \
  | jq '[.packages[].manifest_path | select(test("/crates/[^/]+/Cargo.toml$"))] | length'

# Every documented row -> source files and working-tree Rust LOC.
bun -e 'const s=await Bun.file("AGENTS.md").text(); const start=s.indexOf("## The crates:"); const a=s.slice(start,s.indexOf(String.fromCharCode(10)+"## Use fh",start)); const ns=a.split(String.fromCharCode(10)).filter(x=>x.startsWith("| "+String.fromCharCode(96))).map(x=>x.split("|")[1].trim().slice(1,-1)); for(const n of ns){const d="/Users/josh/Developer/control-plane/crates/"+n; const p=Bun.spawnSync(["find",d,"-type","f","-name","*.rs","-print"]); const fs=new TextDecoder().decode(p.stdout).trim().split(String.fromCharCode(10)).filter(Boolean); let loc=0; for(const f of fs){const t=await Bun.file(f).text(); loc+=t.split(String.fromCharCode(10)).length-(t.endsWith(String.fromCharCode(10))?1:0)} console.log(n+String.fromCharCode(9)+loc+String.fromCharCode(9)+fs.join(","))}'
```

**Source audit result (control-plane `src/lib.rs`/`src/main.rs`, unless noted):**

- **CONFIRMED** — `pane-truth`: pane rules, external command output, and two-capture timing are present.
- **CONFIRMED** — `fleet-truth`: fleet sensors and truth-row rendering are present.
- **CONFIRMED** — `fleet-reconcile`: tmux/NTM reconciliation, typed verdicts, and self-test are present.
- **CONFIRMED** — `oracle-compare`: count/set verdicts and unreadable/empty-arm handling are present.
- **CONFIRMED** — `pane-oracle-diff`: agent-pane census and NTM projection comparison are present.
- **DIVERGENT** — `oracle-pane-state-differential`: it compares session:index `BTreeSet` values through `oracle-compare`; no Z3 dependency or Z3 implementation is present. The table row now states the implementation rather than the stale label.
- **CONFIRMED** — `pane-dispatch-ready`: busy, agent, quota, composer, and motion checks feed admission classification.
- **CONFIRMED** — `pane-dispatch-fence`: per-session/per-pane lock acquisition and release are implemented.
- **CONFIRMED** — `composer-typed`: marker/ANSI-aware typed-composer parsing and self-test are implemented.
- **CONFIRMED** — `ntm-fleet-monitor`: typed actions and approval/refusal wave rendering are implemented; the binary does not send.
- **CONFIRMED** — `loop-queue-filter`: runtime-configured, fail-closed queue filtering is implemented.
- **CONFIRMED** — `loop-coverage`: proof levels, loop layers, edge cases, and reuse authorities form a coverage map, not a gate.
- **CONFIRMED** — `refill-idle-panes`: pane survey, refusal classification, recommendation parsing, and bounded assignment planning are implemented.
- **CONFIRMED** — `fast-dispatch`: fresh-verdict admission, free-pane selection, bounded children, and lock/ledger handling are implemented.
- **CONFIRMED** — `tick-dispatch`: ground-truth pane, discovery, readiness, and send decisions are implemented.
- **CONFIRMED** — `loop-driver`: single-instance locking and deadline-bounded driver output are implemented.
- **CONFIRMED** — `loop-tick`: single-pane dispatch decisions, bounded child execution, and lock acquisition are implemented; its standalone `[workspace]` manifest is the inventory caveat above.
- **CONFIRMED** — `fleet-monitor`: observe wait, idle/ready scan, and standing-verdict writing are implemented.
- **CONFIRMED** — `verify-dispatch`: bead-status-only verification and differential CLI behavior are implemented.
- **CONFIRMED** — `dispatcher-deadman`: eligible-work/no-packet watchdog behavior is implemented.
- **CONFIRMED** — `reap-finished-panes`: finished-pane sweep and bounded external probes are implemented.
- **CONFIRMED** — `omp-idle-dispatch`: fail-closed idle-pane dispatch with typed repository/config inputs is implemented.
- **CONFIRMED** — `wired-but-inert-guard`: tracked caller discovery, gate scans, fail-closed empty-scan handling, and diagnostic commands are implemented.
- **CONFIRMED** — `fleet-composite`: four-factor geometric scoring, malformed-input refusal, and diagnostic CLI behavior are implemented.

The four names shared by both repositories were checked explicitly: `composer-typed`,
`fleet-composite`, and `loop-queue-filter` are byte-identical between
`/Users/josh/Developer/control-plane/crates/<name>` and
`/Users/josh/Developer/omp-orchestrator/crates/<name>`; `pane-dispatch-fence` has the same
purpose but differs in both `Cargo.toml` and `src/main.rs` (the target adds
`subprocess-contract`).

This source audit finds one description divergence and the explicit set/inventory mismatches above;
it does not establish runtime correctness, wiring, or future drift.

---

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

8b. **A RETRACTION THAT LIVES ONLY IN A REPORT GETS RE-PROPOSED BY THE NEXT READER. Put it in the
   source, and give it a test.** `%20`, 2026-09-07, in its own words after I refuted its arc
   hypothesis.

   It had proposed a switch-on precondition of *"until `jplf.1` is selectable."* I dropped the edge
   it blamed, measured that `jplf.1` stayed invisible, and found the real cause: `jplf.1` is an
   **epic**, and `br ready` excludes epics fleet-wide — 0 of 32 open epics offered, correctly, since
   a container is not work. **So the precondition was unsatisfiable by construction** — the
   never-fires class, proposed in good faith and about to be shipped as a gate's trigger.

   **What `%20` did with the retraction is the rule.** It did not merely accept it in a report. It
   shipped `SWITCH_ON_PRECONDITION` as a **string in the binary** reading *"at least one NON-EPIC
   arc member is present in `br ready`; never keyed on jplf.1, which is an epic…"*, plus a test
   named `the_switch_on_precondition_is_not_keyed_on_an_epic` **whose job is to keep the REFUTED
   keying named** so nobody re-adopts it. Verified: 4 `SWITCH_ON_PRECONDITION` sites in source, the
   test present.

   **Why the test matters more than the string.** A comment recording a retraction is prose, and
   this file has an entire section on prose retractions being re-adopted — a stale *"this kernel is
   broken"* note licensed hours of hand-rolling after the kernel was fixed. **A test that fails if
   the refuted form returns cannot be read past.**

   The general form: when a premise is refuted, ask **where the next reader will look**. If the
   answer is "the code", the retraction belongs there. A NEGATIVE_EVIDENCE row, a bead comment and a
   commit body are all *findable*; only a failing test is *unavoidable*.

   **NO-CLAIM.** This keeps a refuted form from being silently re-adopted. It does not make the
   replacement correct — `%20`'s new leaf-keyed precondition is measured-satisfiable today
   (`jplf.9` is in `br ready`), but one row is not a moving arc, and it said so.

8c. **AN EXCLUSION RECORD WITH NO CORRESPONDING LIVE ROW IS STALE STATE, NOT CONTENTION — and
   treating the two the same converts a five-second retry into a five-minute wait.** `%20`,
   2026-09-07, measured within five minutes of the rule being written down.

   ```
   [RCH-I005] refused (project_excluded); 'contabo-3' already runs this project
   rch queue --json  ->  active builds: 0
   ```

   **Zero active builds, so the exclusion was attributable to no live build — therefore not its
   own.** It unpinned the worker and the run succeeded on `contabo-1` **first try**. Under the
   previous reflex — *"`active_project_exclusion` means wait for your own job"* — it had waited
   three times earlier the same evening.

   **The discriminator is the PROJECT FIELD in the live queue, not the exclusion message.** If the
   in-flight build is someone else's, waiting is wrong; if there is no in-flight build at all, the
   record is stale and waiting is wrong for a different reason. Only *"the queue names a live build
   of my project"* justifies waiting.

   **This generalises past rch.** Any admission surface that records a reservation separately from
   the work it reserves for can hold a record whose subject is gone — the leaked-lease family, the
   phantom 4/4 slots on an idle box, the `RCH-I005` that survives a killed client. **Ask what the
   record is a record OF, and check that thing directly.**

8d. **A TEST NAME IS A CLAIM, AND A NAME PROMISING A PROPERTY IT DOES NOT CHECK IS THE QUIET FORM
   OF THE CONJUNCTIVE DEFECT.** `%20`, same session, renaming its own test.

   `an_unattributable_session_log_holds_rather_than_guessing` → `..._is_unknown_with_the_unknown_code`.
   In its words: **"'Holds' was decision language on a body that asserts the outcome and the code, so
   the name promised a property it never checked."**

   Rule 7b catches the loud form — a name joining two properties with `and` is a conflated assertion
   advertising itself. **This is the quiet form: a name asserting a property the body never
   touches.** It is worse in one respect, because a reader scanning names for coverage counts it as
   covered.

   **The check is mechanical: read the name, then read the assertions, and confirm the name names
   only what the body asserts.** `%20` verified nothing was lost before renaming — the decision half
   was already covered by a separate across-every-shape leg carrying that exact evidence row.

8e. **THE LANE SENDS YOUR WORKTREE FOR TRACKED PATHS ONLY — so `RCH-E410` has TWO causes, and
   rule 8 above applies ON THE REMOTE LANE.** Measured 2026-09-07 by `%19` and `%20` independently,
   from opposite ends of one fleet-wide outage. Every pane's remote build refused for roughly an
   hour with `RCH-E410 DependencyPreflightMissing` — *"remote dependency preflight found a missing
   required path"* — naming a path most of them had never heard of.

   **Cause A — a declared entrypoint that does not exist yet.** `%20` watched the refusal WALK:

   ```
   retry 1   missing crates/gate-runner/src/main.rs
   retry 2   missing crates/gate-runner/tests/roster.rs
   retry 3   COMPLETE -> ran
   ```

   `rch`'s preflight requires **every declared entrypoint to exist**, while **cargo's own resolution
   is lazy** — it does not need `tests/roster.rs` until you build that target. So a manifest
   declaring `[[bin]]` and `[[test]]` paths before the files exist **passes `cargo metadata`
   locally and refuses every remote build**, and the refusal walks from one declared path to the
   next as the author writes them. That is sharper than this file's earlier *"a `Cargo.toml` without
   a `src/`"* framing, which describes only the first step of the walk.

   **Cause B — the path exists but is UNTRACKED.** `crates/gate-runner` had all its files on disk,
   `cargo metadata --offline` loaded clean, and `git ls-files` returned **0**. `rch` ships an
   overlay built from git, so an untracked directory does not travel.

   **THE BOUNDARY IS TRACKED vs UNTRACKED — NOT INDEX vs HEAD.** I posed exactly that question and
   declined to guess; `%19` settled it by experiment and **refuted its own first conclusion:**

   ```
   experiment 1   staged, deliberately NOT committed (absent from git ls-tree -r HEAD)
                  -> lane exit=0, 1 passed        the worker ran a target in no commit
                  -> concluded "the overlay archives the INDEX"   <- WRONG

   experiment 2   the negative control it nearly skipped:
                  index holds 1 #[test], worktree holds 2 (the second UNSTAGED, panicking)
                  -> lane: "running 2 tests", exit=101, 1 passed; 1 failed
                  -> AN INDEX ARCHIVE WOULD HAVE RUN ONE TEST
   ```

   ```
   untracked path                       does NOT travel     <- the outage
   staged (A )                          travels
   UNSTAGED edit to a git-known path    travels
   ```

   **So `git add` alone is the minimal fix** for cause B — no commit required — though committing is
   still right, because a tracked-but-uncommitted crate is invisible to a fresh clone.

   **AND THE CONSEQUENCE WORTH MORE THAN THE UNBLOCK: A LANE FIGURE DESCRIBES YOUR WORKTREE, NOT
   `HEAD`.** Rule 8 says `cargo` reads the WORKTREE while a sha names a TREE. **That holds on the
   remote lane too** — which everyone here, including me, assumed the clean overlay removed. It does
   not. A grader citing an on-lane `exit=0, N passed` against a sha has **still mixed two tree
   states** unless it pinned the tree, exactly as if the run were local. In a five-agent shared
   checkout the worktree is constantly someone else's.

   **`%19`'s own summary is the reusable half:** one experiment gave a plausible mechanism and a
   correct practical conclusion; the second refuted the mechanism while preserving the conclusion.
   Stopping at one would have published *"rch archives the index"* — false, and it would have
   licensed the belief that unstaged edits are safe from the lane. **The negative control is the
   whole reason the answer is right.**

   **This is the THIRD instance of the glob-member hazard and its worst variant.** The first two
   (`response-envelope-check` 23:29, `zz-planted-dup` 05:57) broke workspace **LOADING** — loud,
   local, instant. This one loads perfectly and breaks only **TRANSFER PREFLIGHT**, so it is
   invisible from the pane that caused it and reaches peers as a refusal naming an unfamiliar path.
   And this file's existing ruling landed exactly as written: *"a gate author is precisely the agent
   most likely to create one."* The author was mid-`fsu7`, building a gate.

8f. **`RCH-I005 project_excluded` HAS THREE CASES, NOT TWO — and `rch queue` cannot tell you which.**
   Rule 8c established that an exclusion with **no live row** for your project is stale state, not
   contention: unpin and go. `%8` measured the third case 2026-09-07.

   ```
   no live row for the project              STALE      -> unpin and go            (8c)
   live row that is YOUR OWN build          CONTENTION -> wait; a second request duplicates it
   live row that is a PEER's build, same
     project, different pane                CONTENTION -> REROUTE to another worker
   ```

   `%8` found `contabo-3` genuinely running `omp-orchestrator-38cf50d1` —
   `cargo test -j 2 -p gate-runner --test index_probe`, which was **`%19`'s staged-file
   experiment**. It rerouted without waiting, `contabo-1` returned `exit=0`. **Waiting would have
   been wrong**: it was not its own build, so there was nothing to duplicate.

   The trap is that **the queue row does not name the pane**, so it cannot distinguish case 2 from
   case 3 — which is why this file already records *"`rch queue` cannot tell you whether you are
   your own blocker."* Answer it from your own knowledge of what you launched, not from the row.

8g. **A BROADCAST IS A SNAPSHOT AND CARRIES NO TIMESTAMP A READER CHECKS.** `%20`, self-reported
   2026-09-07: it told three panes that only `src/lib.rs` existed under `crates/gate-runner`, and by
   the time the message sent the owner had written `main.rs`. **True when measured, false when
   sent** — it had caught a peer mid-write.

   Its own framing is the rule: the same stale-premise class this file records on beads, *"committed
   by me in a broadcast, where it is worse because a broadcast carries no timestamp a reader
   checks."* A bead comment sits beside its own history; a broadcast arrives as present tense. **So
   a broadcast about a live tree must name its measurement time and tell recipients to re-measure**
   — which is what the correction did.

8h. **` M` ALONE IS NOT A COLLISION SIGNAL. ` M` WITH NONZERO INSERTIONS IS.** Measured 2026-09-07,
   after a mode-only diff changed two routing decisions in one session.

   The agent write tool sets mode `100755` on `.rs` files it touches. No Rust source here is
   executable, so it is pure artifact — but in a shared checkout `git status --porcelain` is how a
   pane decides whether a file is safe to take, and **a mode-only change is indistinguishable from a
   peer mid-edit.**

   ```bash
   git diff --numstat -- <path>     # "0<TAB>0<TAB>path" => MODE ONLY, safe to take
   ```

   **Two false collisions, both of which nearly stood:**

   - `crates/omp-orchestrator/src/main.rs` was reported as *"carries several panes' uncommitted
     hunks"*, so two `finding` publisher call sites that were **refusing at runtime** were left for
     their owner. Measured `0+ 0-`, byte-identical to HEAD. The fix landed only because the diffstat
     was checked.
   - `crates/asupersync-conformance` showed 2 dirty files, and a pane correctly declined to take an
     unassigned crate someone appeared to be editing. Measured *"2 files changed, 0 insertions(+), 0
     deletions(-)"*. **It was free the whole time**, and the caution cost a round trip.

   **A SWEEP DOES NOT HOLD, AND THE SWEEP'S OWN COMMIT SAID SO.** `2bd4e99` cleared the bit from 84
   files (`0 insertions, 0 deletions`) and its NO-CLAIM predicted regeneration; **81 mode-only dirty
   files existed roughly two hours later, in the same session.** Sweeping again is theatre. The
   durable fix is a commit-time refusal, tracked as `omp-orchestrator-3xva`, and its residual is
   stated there: a commit-time gate cannot stop the write tool, so mode-only rows still appear
   between a write and a commit and this diagnostic stays necessary.

   **The general form is the reusable half:** a status flag reports *that* something changed, never
   *what*. Any decision keyed on `git status` alone — collision, ownership, staleness — is keyed on a
   coarser signal than the decision needs. Read the diff, not the flag.

8i. **RUN A NEGATIVE CONTROL ON YOUR INSTRUMENT BEFORE YOU BELIEVE ITS ANSWER.** Joshua,
   2026-09-07, fleet-wide. **This is the general form of rules 8b through 8h and it subsumes them.**

   Point the instrument at a **guaranteed-absent** subject — `/nonexistent/path/xyz`, a symbol that
   cannot exist, an empty scan set — and read what it returns. **If "absent" and
   "present-and-fine" produce the same output, the instrument cannot answer your question and its
   verdict on the real subject means nothing.**

   ```bash
   <tool> --root /nonexistent/path/xyz ; echo "rc=$?"      # what does ABSENT look like?
   <tool> --root . ; echo "rc=$?"                          # now the real one
   ```

   **If those two are indistinguishable, STOP and fix the instrument before reporting anything.**

   **WHY THIS AND NOT MERELY "NAME YOUR SCOPE".** Joshua's four cases:

   ```
   closure-check given an unreachable root  ->  worst=Pass exit=0, "every interpretable stage is
                                                complete"        VACUOUS, not failing
   registry-check under a skip_except stub  ->  PASS rows that were literally `true`
   a grep scoped to docs/evidence           ->  zero, read as "nothing untracked"
   a grep of config.toml for worker tags    ->  zero, read as "no darwin tag" (it is in workers.toml)
   ```

   **Naming the scope catches the last two. Only a negative control catches the first two, because
   there the instrument RAN and returned a confident green.** This is the L4 "gates proven to trip"
   discipline aimed at **any diagnostic binary**, not only at gates. **An empty scan set is an
   ERROR, never a pass. A verdict about a subject the tool could not read is not a verdict.**

   **IT INVALIDATED A DOCUMENT WITHIN AN HOUR OF ITS BEING COMMITTED.**
   `docs/plan/DISPATCH-CHAIN-FORKS.md` reported six lifecycle stages `unknown`:

   ```
   calls(zzz_cannot_exist_fn, also_absent_fn)   ->  NO VERDICT EMITTED
   calls(run_cycle, reap_finished_panes)        ->  NO VERDICT EMITTED   <- IDENTICAL
   ```

   Six rows carried zero information. Cause: `reap_finished_panes` and `ack_stage` are **crate**
   names, not functions, and two further probed symbols — `prepare_bead`, `DispatchPermit` —
   **return `grep -c` 0 anywhere in the repo. They were invented.** Re-probed with real symbols and
   both controls, **nine of eleven stages came back `confirmed`** — the chain was more wired than
   the document claimed, and the entire error was in the instrument.

   **A SECOND INSTRUMENT IN THE SAME TABLE WAS EQUALLY BLIND, and its control is the reusable
   one:**

   ```
   receiver_receipt::  (qualified, known-used)   7
   zzz_absent_crate::  (guaranteed absent)       0
   ```

   **A crate used UNQUALIFIED reads identically to an absent one under a `crate::` pattern.**
   `dispatch_claim_fence` and `dispatch_silence_watch` appear only as `use` lines and were nearly
   reported as BUILT ≠ WIRED, while the names they import are called bare — `authorize`,
   `clears_pending_dispatch_intent` ×4, `SilenceVerdict` ×8.

   **AND IT IS EXPRESSIBLE AS A TEST, WHICH IS STRICTLY BETTER THAN A HABIT.** `%20`, proving
   `m0c`'s amended `2b`, built a differential whose second arm is deliberately **not** the reporter
   — *"a reporter compared against itself agrees by construction"* — plus a leg asserting the two
   arms **MUST DISAGREE** on a known-bad input. If they ever agree, arm two has become a copy of arm
   one **and every equality in the suite is decoration.** A differential whose arms share an
   implementation is an instrument with no negative control; that leg *is* the control, in-tree and
   permanent.

   **The same rule applies to a tool's own output fields.** `rch queue` rendering `project` as `?`
   is **UNREADABLE, not absent-of-rows** — `%20`'s distinction, and it would have justified the
   opposite dispatch decision. A field you cannot read is not a field whose value is empty.

8j. **`fh suggest` NEVER RETURNS EMPTY, SO A SUGGEST ROW IS A CANDIDATE, NOT A HIT.** Measured
   2026-09-07 by `%19` and reproduced by me within the hour. It is rule `8i` aimed at the two tools
   Joshua directed the fleet to use.

   ```
   fh suggest "zzzqqq nonexistent xyzzy plugh frobnicate"
     -> 3 confident ranked rows about "nonexistent file" tests    NO EMPTY STATE
   fh search  "zzz_cannot_exist"
     -> [EMPTY] ... no false hit was returned          exit 4     DISCRIMINATES
   ```

   **`suggest` is lexical ranking: it always answers.** So *"it returned rows"* is not evidence that
   anything matched. **Confirm a suggest row with `fh search` — which does return `[EMPTY]` — or
   with `fh why <row-id>`, before citing it.**

   **I cited two `fh suggest` rows as before-you-build evidence for `etyur` and they survived
   confirmation** (`N046` at `ledger:franken-harvest.md:721`; `verify_binary_runs` at
   `meta_skill/src/updater/mod.rs:620-668`, both row-1/row-3 exact under `fh search`). **They
   survived by query quality, not by method** — the same verb gave `%19` pure noise. The
   discriminator is `search`, and I had not run it.

   **`fh` also reports its own freshness and its own input drift, and both change what a row means:**

   ```
   fh health        [RED] digest_missing_today / digest_stale -- the harvest did not run today
   fh doctor --json DRIFT exit 5  ACTIVE_GENERATION_PRODUCT_INPUT_DRIFT  (dirty source input)
   ```

   `%7`'s scoping is the right one: **treat fh as retrieval and provenance context, not a clean repo
   grade.** A stale row is still evidence; **its age is part of the citation.**

8k. **`ripwire` EMITS ONE LINE, SO EVERY LINE FILTER DELETES THE WHOLE PAYLOAD — and a MISS IS NOT
   AN ABSENCE.** Measured 2026-09-07 by `%19` (four times, while it began diagnosing the tool) and
   independently by me on `--exemplar`.

   ```
   ripwire crates --uses=run_crate | grep -v ...    -> empty, FOUR TIMES
   raw                                              -> <u role="call"
                                                       p="crates/gate-runner/src/main.rs:178"
                                                       in_id="main"/>
   ```

   **`wc -l` is 0 because the payload is one line.** `head`, `grep -v`, and `sed` line filters all
   destroy it. **Read raw, or parse the XML.** My own `--exemplar` grep produced two meaningless
   fragment lines the same way.

   **Three further measured properties, each of which changes a conclusion:**

   1. **A name living only inside a MACRO STRING ARGUMENT is invisible to `--uses`.** In `uds`,
      `--uses=artifact_unchanged` → EMPTY while grep found **21** hits, with the positive control
      passing. **In a repo whose contract is typed refusals, the detector names ARE the API and they
      all live in macro strings.** The follow-up is `ripwire --grep`, **not** bare grep — it
      attributes each hit to its **enclosing symbol** (`in=`), and `unindexed_hits=` is the
      macro-string check built into the verb.
   2. **The legend is 77–95% of every response and does NOT amortise.** `%7` found the lever:
      **`--legend=compact`.** Prefer ONE well-chosen query over three exploratory ones.
   3. **It indexes what it FINDS, including scratch and vendored trees.** Measured here by `%19`:
      **8,570 `.rs` outside `crates/` against 475 inside — 18×**, from a vendored `.rch-tmp/` and
      target dirs. **An unscoped `ripwire .` in this repo is meaningless.** Scope it, and **state
      your scan set whenever you publish a count.**

   **AND IT DISTINGUISHES EXERCISED FROM CONSUMED, WHICH A GREP CANNOT.** My grep for `etyur` said
   *"`derive_checks`: 0 occurrences in `main.rs`"*. `%19`'s `--uses=derive_checks` said **5 uses,
   every one in `tests/roster.rs`, each attributed to its test fn, ZERO in any `src/`.** Strictly
   stronger — and it is exactly `N046`'s distinction: *"invocations from /tmp prove testing, not
   consumption."*

   **AND AN `fh` CAPABILITY ROW'S VERDICT SHAPE AND ITS MECHANISM ARE SEPARABLE. THE MECHANISM MUST
   BE RE-VERIFIED AGAINST THIS REPO'S OWN LINTS BEFORE IT IS COPIED.** Measured 2026-09-07: I cited
   `meta_skill/src/updater/mod.rs:620-668` `verify_binary_runs` as the arsenal precedent for
   `etyur`'s "a declared bin that cannot be exec'd must ERROR". `%19` read it **on the mirror, as
   instructed** and found:

   ```rust
   .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
   let status = loop { match child.try_wait()? { Some(s) => break s, None => { … sleep(25ms) } } };
   ```

   **That is the undrained-pipe deadlock pattern verbatim, and this repo ships
   `crates/undrained-pipe-lint` whose entire job is to refuse it** — its predicate at `src/lib.rs:9-10`
   is *"a Command builder that sets BOTH stdout and stderr to `Stdio::piped()`, whose handle is then
   polled with `try_wait()` in a loop"*, quoting the asupersync contract above.

   **It is SAFE where it lives and UNSAFE where I pointed it.** `--version` emits a few bytes so the
   pipe never fills; a declared `[package.metadata.gate]` check is an **arbitrary command with
   arbitrary output** — exactly the 64 KiB case. **Copying it would have shipped the deadlock into
   the check runner, and the tell reads as a SLOW gate rather than a WEDGED one**, which is the
   harder failure to diagnose.

   **Keep the verdict shape, replace the mechanism:** `subprocess_contract::bounded_output` is
   verified to drain — `src/lib.rs:216` spawns a `stdout_reader` thread, `:160` `join_reader`,
   `:254-255` joins both. In-repo bounded-spawn exemplars, from `ripwire --callers=bounded_output`
   (52 callers, `hop_tested=17`): `crate-soundness-verify::run_binary src/lib.rs:262`,
   `cargo-lane-budget::run_bounded:220`, `admission-reason::spawn_timeout:152`, all tested.

   **`fh` ranked the row correctly and the row is correct in its own crate. The defect exists only
   at the boundary where it would be reused, so no amount of ranking quality could surface it.**
   `fh suggest` tells you **where to look** and cannot tell you **whether to copy** — which is why
   the instruction is *"grep the mirror"*, not *"cite the row"*. `%19` followed the instruction I
   had given and not followed myself.

8l. **A `success:false` FROM `ntm --robot-send` IS NOT PROOF OF NON-DELIVERY, AND THE RETRY MAY
   DOUBLE-PASTE.** Measured 2026-09-07 by `%19`: a callback reported
   `"success": false, "1 of 1 sends failed"` from `tmux send-keys`, then **succeeded byte-identically
   on immediate retry**, with `blocked: false` and no redaction findings.

   **So the sender's own failure report is indeterminate in both directions** — the first send may
   have landed, in which case the retry pastes a second copy into the composer. **A partial paste is
   the worse half of that outcome**, because a truncated packet reads as a malformed instruction
   rather than as a transport fault.

   This is the mirror of `cp-z42vu`, already recorded above, where a send returned `success:[4]`
   while the packet never arrived. **Both directions are the signature of an unacknowledged
   transport**, and the ACK comment is the only evidence the design admits on the tmux path. **Read
   the receiver, not the sender's verdict** — and when you retry, say so, so a doubled packet is
   attributable.

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

Measured 2026-09-05/06. Every row cost someone real work in one session; five of the seven were
the conductor's own faults, corrected only because a second instrument disagreed. These are not
style notes — each is a case where the OBVIOUS read of a tool reports the OPPOSITE of the truth.

|surface|the obvious read|what it actually does|the correct test|
|---|---|---|---|
|`am file_reservations reserve`|nonzero on refusal|**exits 0** with `granted: []` and `conflicts` naming the holder|`len(granted) > 0`|
|`jq 'if .granted then'`|false on `[]`|**an empty array is TRUTHY in jq**, so a refusal prints "granted"|test the length, never the array|
|`inbox-monitor --watch`|zero on success|**exits 12** on a settled watch — the nonzero IS the finding|read `verdict`, not the exit code|
|`br show <short-id>`|exact match|**suffix-resolves** — `…-ipg.18` returns `…-omp-coverage-mission-ipg.18`|read `.id` back before comparing surfaces|
|`br list --json`|carries comments|**no `comments` key at all**; a classifier keyed on it reports zero for every row|read `.beads/issues.jsonl`; control on a bead you know|
|`$?` after a pipe|the subject's status|the **pipeline's last** command — `cmd \| head` reports `head`|redirect to a file, capture separately|
|`cargo test -p X`|the crate's tests|**that target only** — integration targets are separate, so a count can be honestly low|name the target, or sum them|

**The unifying fault: the exit code is asked to carry a status it cannot express.** Two rows above
fail in OPPOSITE directions — `reserve` exits 0 on refusal, `inbox-monitor` exits nonzero on success
— so **no single exit-code convention is safe across our own tooling.** Read the payload.

### The failure this prevents is not a wrong number, it is a confident wrong CAUSE

In every instance the instrument produced a plausible story and the story was believed:

- A `jq` truthiness bug printed `RESERVED for pane1` while `CloudyGrove` held the lease. **One step
  from two agents editing one file.** What stopped it was an unexplained exit code from the
  surrounding pipeline, not the probe.
- `br` suffix-resolution vs Python exact-matching made six beads look absent from the JSONL. The
  conclusion published was **"the JSONL is stale"**, followed by a pointless flush that correctly
  answered *"Nothing to export."* The JSONL was never stale.
- `br list --json`'s missing `comments` key made a classifier report **0 reapable beads out of 119**.
  The positive control that caught it: `eg0m` has **17** comments. A reader returning 0 for `eg0m`
  is broken; the data is not empty.
- `$?` after a pipe reported a gate exiting **0** when its true exit was **1** with 37 real rows.
  A correctly firing gate was one sentence from being graded as non-firing.

### The offload lane is not the only lane, and a timeout is not a verdict

`rch`'s workers are **Linux x86_64**. That is a property of the OFFLOAD FLEET, not an absence of a
build lane. `RCH_CARGO_WRAPPER_BYPASS=1 cargo …` is the sanctioned local path, it produces
`Mach-O 64-bit executable arm64`, and it built and installed five codesigned binaries in one
session. Measured contrast: `path-literal-guard` returns `10 passed` in **0.00s** locally where the
same suite hit an RCH `queue_timeout` at **300s**.

This stale premise cost real work **twice in one session**: two panes refused to install, believing
"no Mach-O artifact lane" existed, and a grading batch labelled **six** offload timeouts as `GAP`.

**A grading verdict vocabulary needs four values, not three: PASS, GAP, STALE, and UNKNOWN.**
`GAP` means the work fails its acceptance. **"I could not execute the check" is UNKNOWN and says
nothing about the work.** A batch reporting `PASSED=0 GAPPED=16` where six checks never ran does not
describe a broken codebase — it describes a saturated queue, and it hands the next reader sixteen
verdicts of which six were never measured.

### Agent NAME is not an identity

Two panes signed ACKs as `WildStone` simultaneously; one of them was `RubyGate` in Agent Mail; the
`am` roster separated them only by **model**. `created_by` reads `josh` or `None` on the rows that
matter, so a grader-≠-author check built on it **excludes nobody while reporting success** — a
vacuous filter that routed 13 beads to their own authors. The only key that proved unique was the
**pane id parsed out of the bead's own ACK lines**, and even that is unique only per OCCUPANCY,
which is why `pane-dispatch-fence`'s `PaneIncarnation` exists.

**NO-CLAIM.** This table is a list of measured surprises, not a specification. Every row states what
was observed on one host on one date; none of them was read from the tool's source. `reserve`'s exit
code on a SUCCESSFUL grant is UNMEASURED — only the refusal case was observed — and `br`'s behaviour
when a suffix matches two beads is likewise unmeasured and must not be assumed to error.

---

## A TRANSCRIBED VALUE IS STALE BY DESIGN — FIVE SUBSTRATES, ONE SHAPE

**Measured across two repositories on 2026-09-07. Five instances, five different substrates, one
defect: a claim that transcribes a value instead of binding to something that RE-DERIVES it.**

**Substrates 1–4 are citations whose TARGET moves. The fifth is different in kind and is the worst,
because it defeats re-running: a figure whose SCOPE moves while the command and the tree hold
still.** It has its own subsection at the end.

|substrate|the instance|why it went stale|
|---|---|---|
|**FILE LINES**|`CONTRACT.md`'s superseded-by pointer said `:101`, corrected to `:116`, and **the correction invalidated itself in the same edit** — the inserted lines pushed the target to `:124`, then `:132`, then `:154`. Fixed at `13fc201`|any edit above the target shifts it, and the edit most likely to be made is the one fixing the pointer|
|**BINARY VERSIONS**|AGENTS.md's 42-method RPC census anchors on `let w=async(v)=>` … `},E=new KWt`. **Both return 0** at the installed `omp/18.1.13`; the gate pins `18.0.11 / a95635ad… / 19,803,745 bytes`|**`uca service install` runs a THREE-HOUR auto-updater** across `claude, codex, agy, grok, omp, muse`. The pin is stale by design, not by neglect|
|**PROSE PREMISES**|control-plane's `AGENTS.md:12` asserted three root files "never existed", measured 2026-09-03. **All three exist.** The stale entry propagated into a dispatch, then a worker restated it back to its author as established fact. Fixed at `daab4fe`|a dated measurement embedded as a standing claim, with the date discarded|
|**UNREACHABLE ANSWERS**|`crates/omp-surface-consumption/src/lib.rs:18` **had already recorded** the vanished anchor, and `:13-14` the version drift, five days before two agents independently re-derived it. `cargo-bin: 1`, **`PATH: ABSENT`**|the citing document could not reach the crate holding the answer. The operator-surface defect causes measurable duplicate work, not just inelegance|

**THE REMEDY IS THE SAME IN ALL FOUR: bind the claim to something that RE-DERIVES, or stamp it with
a fetch time and an expiry.** Never transcribe a value that another process owns.

What that looks like concretely, each verified in this repo:

- **Cite a searchable string, not a line number.** `grep -nE '^\*\*S1 IS AUTHORIZED TO BUILD'` —
  and **anchor it so the pointer is not its own hit.** Unanchored returned **3**, two being the
  pointer quoting the phrase.
- **Anchor a binary probe on WIRE CONTRACT, not on minified identifiers.**
  `omp-surface-consumption:64` gets this right: `ANCHOR_METHOD = "negotiate_protocol"`. A protocol
  method name survives a rebuild; `let w=async(v)=>` is a minifier's variable name and does not.
- **Stamp every figure with its measurement time and say which TREE it came from.** `cargo` reads
  the WORKTREE; a sha names a TREE. A grade citing both has silently mixed two states.
- **When the answer is in a crate, INSTALL the crate.** An answer nobody can invoke gets re-derived.

### THE COROLLARY THAT COSTS THE MOST: SCOPE A VOIDING RULE TO WHAT IT ACTUALLY GOVERNS

**Measured the same day, and it was my own error.** Joshua's binding rule is **ALL BUILDS MUST TAKE
THE CONTABO LANE.** I broadcast it to five panes as voiding *"any figure derived from"* a local run.

**That over-applies, and control-plane pane 0 caught it before it did damage: THE BINDING BINDS
BUILDS.** The two results that actually moved the product that night involved **no cargo at all** —
`uds-dc-stamp-ne-x35` is a python predicate reading the working tree, and `uds-kii.2` repaired a
`sed` block inside a markdown contract. Voiding non-build figures would have discarded the only two
rows that moved `dag_closure_scorecard.tsv` (PASS 1 → 2, UNRUN 46 → 45).

**A voiding rule is itself a claim and inherits every rule above.** State the predicate it voids on,
not a vibe about provenance: *builds and their test figures*, not *everything measured locally*.
An over-broad retraction destroys good evidence and is harder to undo than a stale figure, because
the good evidence does not come back when the rule is narrowed.

**NO-CLAIM.** Binding to a re-deriving probe makes a claim *self-correcting*, not *correct*. A
probe can re-derive the wrong thing forever — `%20`'s **"I measured TOKEN PRESENCE and reported
REQUIREMENT EQUIVALENCE"** is exactly that: an instrument internally consistent and pointed at the
wrong object. Re-derivation fixes staleness; only a positive control and a known-bad leg fix aim.

### I MEASURED A SUBSET AND GENERALISED TO THE POPULATION — 19 of 44, not 19 of 19

**Measured 2026-09-07 by `%20`, correcting me. My own unstated-denominator defect, committed while
documenting the class.**

I reported *"all 19 recorded HD decisions carry a real decision, so 'awaiting a human' is almost
always false"* and built a bead-triage rule on it. Re-measured over `docs/decisions.jsonl`:

```
rows                                  56
distinct HD ids                       44        <- THE POPULATION
ANY row carries a decision            19        HD-0001..HD-0018, HD-0033
NO row carries a decision             25        HD-0019..HD-0032, HD-0034..HD-0044
of the 19 decided, carrying an execution receipt   ZERO
```

**My claim is exactly right about those 19 and does not generalise to a population of 44.** So the
conclusion **INVERTS**: *"awaiting a human"* is **genuinely true for 25 of 44 ids.** The inversion I
found holds for `HD-0009` specifically — which happened to be the one that mattered, which is
precisely why the over-generalisation survived.

**AND THE MISSING FOURTH STATE IS THE LARGEST ONE: `ExecutionOwed`.** Every recorded decision is
unexecuted — **zero execution receipts across all 19.** `HD-0008`'s *"push it"* from 2026-09-02 is
the archetype.

> **A bead in `ExecutionOwed` is WORK and must NEVER be excluded as a human hold** — which is
> exactly what a hand reading of a human-sounding title does. I did it twice in one pass: a
> prose-anywhere matcher over-caught **9** (seven P0), then a title-only matcher over-caught **8**
> more. Both measured token presence and reported pending-decision state.

The five states a triage predicate must distinguish, per `%20`'s runner (`fe291df`):
`DependencyBlocked` · `TrackerBlocked` · `ExecutionOwed` · `AwaitingHumanDecision` ·
`Unclassifiable`. **Count only the two that are work wearing a hold; exclude
`AwaitingHumanDecision` BY A NAMED PREDICATE; make `Unclassifiable` an ERROR** so a bead naming an
unknown id is never silently excluded.

### TWO CRITERIA OVER ONE SET MUST NOT DISAGREE ON THE DENOMINATOR

**Measured 2026-09-07: R3 printed `144` where R2 printed `138` over the same beads**, because R3's
population did not exclude the six layer gates. **Neither number was wrong in isolation and only
running them side by side revealed it.**

A criterion's denominator is part of its claim. Two criteria scoped to one set and reporting
different populations means at least one is measuring something other than what it names — and
**both can pass while disagreeing**, which is the failure mode: nothing in either runner compares
them.

**`0 of 0` MUST BE PRINTED WITH ITS POPULATION.** `%20`'s live R3 reads
`0 of 0 blocked (population 138)`, because **a criterion reading 0 from an EMPTY population cannot
otherwise be told from one reading 0 from a healthy one.** That is the anti-vacuity rule stated as
an output format rather than a test.

**AND FIXTURES CAUGHT TWO DEAD GUARDS IN THE AUTHOR'S OWN RUNNER, second pass running:**

- **The ledger parser accepted any row with an `id` as a decision id.** Pointed at a BEAD file it
  yielded one bogus id, `seen_ids` was non-empty, and **the anti-vacuity guard silently did not
  fire.** Fixed with `re.fullmatch(HD-\\d{4})`.
- The population/denominator disagreement above.

Together with R2's known-good leg catching a `138 of 138` false FAIL, that is **three instrument
defects caught by mandatory legs in two passes** — every one invisible to a clean run. **Prefer the
report that names its own instrument failures over the one that reports green.**


### A FIX'S OWN MUTATION OUTPUT IS NOT A DESCRIPTION OF THE PRE-FIX TREE

**Measured 2026-09-07. The first time tonight a wrong line number came from EVIDENCE rather than
from age — and it is the most dangerous variety, because it arrives with a passing/failing verdict
attached and therefore looks authoritative.**

A `gate.yml` fix (`99295c8`) shipped a mutation leg that removed the restored job key to prove the
detector bites. The detector printed
`DUPLICATE_KEY scope=jobs/head-compiles-as-committed key=runs-on lines=[154, 184]`, and **`184` was
carried forward as the pre-fix duplicate.** Measured against both real trees:

```
PRE-FIX  99295c8^   154 runs-on / 155 steps  +  171 runs-on / 172 steps   <- the actual duplicate
                    strict loader: DUPLICATE KEY 'runs-on' at line 171
HEAD     99295c8    154 runs-on / 155 steps  +  184 path-literal-guard:   <- a RESTORED JOB KEY
                                                185 runs-on / 186 steps   <- that job's own body
```

**`184/185` was never a duplicate pair in any commit.** It is an artifact of the mutation's
*synthetic intermediate state*: a 13-line explanatory comment had already been inserted, then the
job key removed. **That state exists in no tree.**

**AND THE DANGEROUS HALF:** in the current tree, line **184 is `path-literal-guard:`** — the
restored key. A reader who took "184/185" as "the duplicate to remove" would **delete the restored
job and re-create the original defect**, and the same three detectors would go red exactly as
before — **so it would read as a regression rather than a re-introduction.**

> **Cite a mutation for DIRECTION — it went red, it came back green, the restore was
> byte-identical. NEVER for LOCATION.** A mutation deliberately perturbs the file, so its line
> numbers are the least citable in the whole record.

**What to cite instead, in the form that re-derives:**

```
pre-fix tree     99295c8^
strict verdict   DUPLICATE KEY 'runs-on' at line 171     (the loader quotes its OWN line)
cause            no `path-literal-guard:` job key existed; two jobs had collapsed into one
remedy           RESTORE one job key — never delete a block; both blocks are real jobs
fix              99295c8
```

**The cause sentence re-derives from any tree; the line numbers do not.** Let the strict loader's own
message carry the line, exactly as `13fc201` did for `CONTRACT.md`.

**AND THE REMEDY WAS INVERTED BY THE SAME ERROR.** Both beads describing this defect proposed
deciding "which block is canonical" and deleting the other. **Neither block was redundant** — the
root cause was a *missing* job key, so the correct fix is a one-line RESTORE. **A deletion would
have removed a real CI job and gone green**, which is the failure the beads existed to prevent.

**Third line-pinned citation to be wrong in one session** — `AGENTS.md`'s `:46/:52`, `m0c`'s title,
and this one. The first two went stale; this one was **born wrong from a correct measurement of the
wrong tree state.**


### FIFTH SUBSTRATE — A FIGURE WHOSE **SCOPE** MOVES WHILE COMMAND AND TREE HOLD STILL

**Measured 2026-09-07 by `%19`, filed as `omp-orchestrator-mmt4` (P0). The first four substrates are
citations whose TARGET moves. This is a figure whose DENOMINATOR moves — and it defeats both clauses
of our grading standard at once.**

Three answers, one command, all real:

```
local, fail-fast       (bypass, darwin arm64)    11 targets    33 passed /  3 failed
lane,  fail-fast       (contabo-2, exit 101)      9 targets    23 passed /  2 failed
lane,  --no-fail-fast  (contabo-2, exit 101)     52 targets   251 passed / 55 failed
```

**`cargo test` stops at the first failing TARGET, and the stop point is environment-dependent.** The
two truncated figures are **not** bigger and smaller versions of one measurement — they are
different **PREFIXES**.

**The mechanism, corroborated at source level by pane 1:** `crates/no-shell-gate/tests/` holds **43**
integration targets (plus 6 `src/bin` and 1 lib), and **`artifact_provenance.rs` is alphabetically
FIRST while `bead_shape.rs` is THIRD.** On the lane `artifact_provenance` fails, so execution stops
and `bead_shape`'s three label failures **never run**. Locally it passes, so they do. **The stop
point is filename ordering crossed with which target fails in this environment.**

**Why this is P0: it survives both halves of "a grade re-runs, and states the tree."** A grader can
re-run the exact command, on the exact tree, in the mandated lane, and cite a figure that silently
omits **43 of 52 targets**. This is the FOURTH quantity in this repo wearing the word "tests", and
the only one indistinguishable from the full aggregate by inspection.

#### RULING (pane 1, 2026-09-07). Both halves of the proposed choice, split by purpose.

1. **An ACCEPTANCE leg MUST cite a NAMED TARGET** — `cargo test -p <crate> --test <target>`. A named
   target **cannot truncate**, is cheap, and is what a bead's acceptance is actually about. `%20`'s
   on-lane re-runs already did this correctly (`--test starvation_taxonomy` → 3/0,
   `--test empty_staged` → 5/1).
2. **A CRATE-HEALTH claim MUST carry `--no-fail-fast` AND the target denominator** — "251 passed /
   55 failed across 52 targets", never "251 passed / 55 failed".
3. **A bare `cargo test -p <crate>` figure is INADMISSIBLE as evidence.** It is a prefix of unknown
   length whose end is decided by alphabetical filename order crossed with environment. Not
   "discouraged" — inadmissible, the same standing as the retired "81 JSON-RPC methods, 17 used".
4. **A `--no-fail-fast` failure count is a COUNT, not a DEFECT count**, until environment-caused
   failures are split out. `%19` named this against its own figure: `this_repo_is_clean`,
   `every_in_repo_line_cite_names_a_line_that_exists` and
   `hook_validates_staged_bytes_not_a_dirty_worktree_copy` need `.git`, `.beads`, `crontab` or a
   clean worktree, **none of which `rch` syncs**. **Nobody may cite 55 as a defect count**, its
   author included.

**AND THE AUTHOR COMMITTED THE DEFECT WHILE FILING A BEAD ABOUT DISHONEST COUNTING.** `3ae3`'s
description reported "33 passed / 3 failed across 11 targets" as `no-shell-gate`'s state; the crate
has **52**. A fifth of the suite described as the whole — the unstated-denominator defect. `3ae3`'s
*defect* survives (absolute-count ratchets over live tracker data is a source property no
environment changes); its *figures* do not.

**Corollary for the CONTABO binding:** three separate panes have now measured live-`.beads` tests
going RED on the lane because **`rch` syncs source without `.git` or `.beads`**. That is a
structural consequence of the binding, not a defect in those tests. An absent mirror is
**UNMEASURED**, never "the oracle is vacuous" — and the discriminator must be POSITIVE: a tree with
no `.git` is a synced worker copy and cannot answer; a tree that IS a checkout with no mirror still
FAILS. Absence alone never satisfies.

**NO-CLAIM.** This ruling makes a truncated aggregate *detectable*, not impossible. `--no-fail-fast`
still cannot separate an environment failure from a real one — item 4 is a disclosure requirement,
not a mechanism — and **libtest CAPTURES stdout for a PASSING test**, so a green suite cannot itself
distinguish "ran and passed" from "declined as UNMEASURED" without `-- --nocapture`.


---

## A DENIED OR ERRORED PROBE IS *UNKNOWN*, NEVER A NEGATIVE RESULT

**Measured 2026-09-02, and it is the ugliest root cause of that session.** An agent checked whether
the `am` CLI carried an auth token with `env | grep -i -E 'agent_mail|AM_'`. **`dcg` DENIED the
command as a policy violation.** The agent never retried, then asserted *"the CLI carries no token"*
as measured fact, built a two-authority architecture on it, wrote it into a crate's module docs, and
broadcast it — where it was adopted as house doctrine.

Every part of it was false. `printenv HTTP_BEARER_TOKEN` -> **SET, 65 chars**; `AGENT_MAIL_TOKEN` ->
**SET**; both read at `mcp-agent-mail-cli/src/lib.rs:82140`. The CLI had authenticated via the
environment the entire time. `printenv` was available throughout.

**The refusal was not evidence. It was the absence of evidence, wearing evidence's shape.** **A tool
refusal, a non-zero exit, an empty result, and a policy denial are all UNKNOWN.** The honest moves
are: retry differently, or say unknown. Asserting the negative is the one move that is never
available.

**AND THE THING THAT FALSE PREMISE WAS INVENTED TO EXPLAIN IS NOW UNEXPLAINED AGAIN.** `am agent
start` reported "no listener on 127.0.0.1:8765" while `curl /health` returned `status: ready` and
two robot calls returned live data. The two-authorities story accounted for it; the story is false,
so **the contradiction is OPEN and must stop being cited as answered.** A retracted explanation
does not leave the thing it explained explained.

**What survives, measured at the shipped tag `v0.3.31`:** the CLI calls the daemon **by default** —
`mcp-agent-mail-cli/src/lib.rs:8911` is `!direct || daemon_reachable`, doc-commented "the default
(non-`--direct`) path always prefers the daemon", so **omitting `--direct` takes the daemon
unconditionally with NO fallback** and the SQLite read is the exception. Its own 401 text at
`:40179` names `AGENT_MAIL_TOKEN`/`HTTP_BEARER_TOKEN` — a CLI that never called the daemon could not
emit that. `/api/` and `/mcp/` are each other's alternates (`:9351-9352`). And `am health` really
does build a throwaway probe SQLite and never contacts the daemon — **that single measurement was
correct; the error was generalising `health` to the entire CLI.**

**A CORRECTION TO THIS SECTION'S OWN FIRST DRAFT, which is the point of the section.** It shipped a
replacement row claiming `--direct` is inverted relative to its help text. **That is also false**,
and a second reader refused it before it could be filed: the installed `--help` reads "Allow a
direct SQLite read only when no daemon is reachable", which **is** the predicate. Help and code
agree; the doc comment names GH#158 (WAL contention). **A retraction is not a licence to publish the
next plausible story** — the replacement needs the same standard as the thing it replaces, and this
one was accepted into doctrine for twenty minutes on nobody's measurement.

### A BORROWED CLAIM INHERITS ITS AUTHOR'S BURDEN

**The rule above covers the diagnosis half. This is the transmission half, and it is how one wrong
claim became house doctrine across three agents in under an hour.**

The denied-probe failure was one agent mis-diagnosing its own measurement. What happened next was a
different failure with a different cure: **two other agents, including this file's editor, repeated
the finding as established provenance without reading the source.** It arrived measured-sounding —
file, line, a predicate, a confident causal story — and that shape was accepted *as* verification.
It was then written into `AGENTS.md`, cited three times as evidence in grades, and broadcast to the
fleet as "the definitive explanation" before anyone opened the file.

**A report is a claim** — the rule this repo already applies to subagents and to bead close reasons.
It applies identically to a **peer**, and it is easier to forget there, because a peer's claim
arrives with the social weight of collaboration rather than the suspicion we reserve for our own
probes. **Restating someone else's finding makes it yours.** Cite it and verify it, or attribute it
and mark it unverified. There is no third option in which you get to hold it as fact because someone
else measured it.

**The measurable tell:** if you can state a claim's file and line but have not opened that file, you
are transmitting, not verifying. The cheap fix is to open it — every one of the four refutations in
that investigation cost one `git show` against a version-matched tree.


### RE-RUNNING THE HOOK AFTER A COMMIT IS A CATEGORY ERROR, NOT A RED FLAG

**Measured 2026-09-02, and it is a trap laid by a correct fix.** Since the empty-index repair, a
standalone `.git/hooks/pre-commit` run returns **3** with `NOTHING_TO_CHECK: no staged files to
check` once the index is empty — which is exactly right, because after a successful commit **there
is nothing staged to check**. The gate is answering the question it was asked.

But the obvious way to double-check a hook — commit, then run the hook again to be sure — now
produces a nonzero exit and a refusal-shaped message, and **reads as a failure that just landed**.
Two agents walked into a version of this tonight, one of them the author of the fix.

**Post-commit, the authoritative evidence is the commit-time `CLEAN: all staged files passed the
multi-gate checks` line.** A later standalone run measures a *different input* — an empty index —
and therefore cannot confirm or refute what the commit did. It is not a weaker check; it is a check
of something else.

**The general form, which is the reusable part:** a gate's verdict is only meaningful paired with
the input it ran against. Re-running a gate against a *different* input and comparing verdicts is
the same error as comparing two `git` figures taken from two trees — and it produces the same
confident-wrong reading. Capture the verdict at the moment of the operation, or re-create the input
before re-running.

### A PEER'S DEFECT REPORT IS A VERDICT ON A TREE. REFUTING IT ON A DIFFERENT TREE IS THE SAME ERROR.

**The rule above is stated about hooks and commits. It applies identically to a PEER'S BUG REPORT,
and that is the harder case, because a refutation feels like verification.**

**Measured 2026-09-07, a genuine near-miss.** `%20` reported a fleet-wide cargo outage: a stray `.`
outside the closing quote at `crates/refill-idle-panes/Cargo.toml:6:137` broke workspace *loading*,
so `-p <crate>` could not dodge it and every cargo command in the repo failed. Pane 1 measured the
diff, found a peer's two dependency additions in flight beside the typo, applied the minimal fix —
period back inside the quote — and broadcast the clearance.

`%19` then measured line 6, got **byte-identical to HEAD** on both sides, saw a `git diff` with
**no `description` hunk at all**, and drafted: *"the accused line is byte-identical to HEAD; the real
diff is two added dependency lines."* **A refutation of a correct diagnosis, one message from
broadcast.**

**The fix had landed between the notice and the read.** `%19`'s measurement was correct and
consistent with a **post-fix tree**; it could say nothing about the pre-fix claim. And the pre-fix
state was **unrecoverable** — the file was uncommitted, so there is no history to diff against. Only
the reporter's own quoted error text survived as evidence.

**The cost of publishing it would have been real and asymmetric:** a correct diagnosis discredited,
its author's next report discounted, and the next outage of that class read as a false alarm. **A
wrong refutation is worse than a wrong report**, because it also destroys the reporting channel.

**The mechanical form:** a defect report is a verdict on a tree at a time. Before refuting one,
establish that you are reading **the same input** — check whether the file changed since the notice
(`git status`, mtime, or ask), and **name the tree your refutation read**. A refutation that does not
name its input is exactly as stale as the claim it thinks it is killing.

**Corollary for uncommitted state, which is where this bites hardest.** A `git`-based check cannot
reconstruct a dirty file's earlier content. When the subject is uncommitted, **the reporter's quoted
error output IS the primary evidence** — treat it as the artifact, not as a claim to be re-derived.
A repair that clears the symptom also destroys the evidence, so the fixer must quote the pre-fix
state in the clearance notice. Pane 1 did quote the byte and the two `sha` values; that is what let
`%19` reconcile instead of escalate.

**NO-CLAIM.** This makes a stale refutation *detectable*, not impossible. Two panes reading the same
tree can still disagree for other reasons — different globs, different anchors, different key shapes
across `br show` / jsonl / `br list`. Naming the tree removes one failure mode from a family this
file records eight other members of.


### A FILTERED DIFF IS NOT A DIFF — and this member of the family drives a DESTRUCTIVE command

**Measured 2026-09-07, self-reported by `%20` against its own remedy.** During the fleet cargo
outage it needed to inspect one manifest line and ran:

```bash
git diff -- crates/refill-idle-panes/Cargo.toml | grep -E '^[-+]description'
```

It then reported the defect correctly — a stray `.` outside the closing quote at `:6:137` — **and
named `git checkout -- <manifest>` as the fix.** That command would have destroyed a peer's two
in-flight dependency additions (`agent-mail-native`, `asupersync`) and desynced them from their own
modified `src/main.rs` and `tests/differential.rs`.

**The distinguishing feature: the evidence was RETRIEVED AND THEN DISCARDED.** The two `+`
dependency lines were in the command's output stream; the filter dropped them before any human or
agent read them. Every other member of this family produces a wrong **number** — `$?` after a pipe,
`git log -S` skipping merges, `grep -c … || echo 0` emitting `"0\n0"`, `rg -c | wc -l` returning the
glob size. **This one produced a wrong BASIS FOR A DESTRUCTIVE ACTION**, which is a strictly worse
failure mode than a wrong figure.

**And prose caution did not save it.** `%20` had already written *"a peer mid-edit could have a
larger change in flight that my fix would clobber"* — correct reasoning about the hazard — and then
let a narrowed `grep` tell it the hazard was absent. **A stated risk does not survive contact with
a filtered instrument**; the filter answers a different question and looks complete doing it.

**The mechanical form:** before any command that discards working-tree state, read the **whole**
diff and the **whole** `git status` for the path. Narrow only to *locate*, never to *decide*. And
prefer the minimal edit over the categorical revert — the fix that shipped was moving one character
back inside the quote, which left every other change intact and made line 6 byte-identical to HEAD
(`sha f4e657007bbd33d1` both sides).

**NO-CLAIM.** Reading the whole diff catches co-located peer work in the *same file*. It does not
catch a peer whose related edits sit in files you did not diff — here, `src/main.rs` and
`tests/differential.rs` were also ` M`, and only a path-scoped `git status` showed them. Whole-diff
plus whole-status, or the check is partial.

### `git show | grep` CONFLATES THE COMMIT MESSAGE WITH THE DIFF — and a claim of absence can be defeated by its own prose

**Measured 2026-09-07. My own claim, caught by `%20`, and the purest instance of the
self-referential-instrument family in this file.**

I wrote *"`%19`'s diff touches `owner` 0 times"* to attribute a lane test failure to the
environment rather than to a peer's commit. **The substance was right; the method was not.**

```
git show 49c7c22 | grep -c owner                                    -> 2   <- includes the MESSAGE
git show --unified=0 --format= 49c7c22 | grep '^[+-]' | grep -c owner -> 0   <- diff only
git diff 49c7c22^ 49c7c22 | grep -c owner                            -> 0   <- diff only
```

**The two hits are in the commit message, and line 59 of that message is the sentence
*"My diff touches `owner` 0 times"*.** So a verification of absence, run with `git show | grep`,
**returns nonzero because the claim's own prose contains the needle it denies.**

> **`git show` is `message + diff`. If you are making a claim about the DIFF, you must exclude the
> message** — `--format=` empties it, or use `git diff <sha>^ <sha>` and never `git show`.

**Why this is worse than the sibling rules:** `git log -S` skipping merges produces a *false zero*,
which reads as absence and is caught by any positive control. This produces a **false NONZERO on a
true absence**, so the instrument appears to *refute* a correct claim — and the more carefully the
commit message documents the reasoning, the more likely it is to defeat the check. **A well-written
commit message is the failure mode.**

Same family, all measured here: `git log -S` skipping merges; `grep -c … || echo 0` emitting
`"0\n0"`; `$?` after a pipe returning the pipeline's status; a doc comment containing the needle it
warns about; a census table naming every gate it checks; and a citation-hygiene scan finding the
specimens inside its own defect reports. **Twelfth instance, and the first where the instrument's
extra input was the author's own explanation.**

**`%20` hit the sibling shape on the same bead, one call apart:** its opening census printed **3**
raw `.output()` sites and was counting the `// omp-orchestrator-3kcl: this was a raw '.output()'…`
**comments documenting the fix.** Comments stripped → **1**, and that one is the `#[cfg(test)]`
helper. **Strip comments before matching** — re-learned by publishing a wrong 3 and catching it in
the next call.

### `git log -S` SKIPS MERGES BY DEFAULT — ITS ZERO IS NOT EVIDENCE OF ABSENCE

**Measured 2026-09-02.** An agent searched for the commit that introduced a string with
`git log -S`, got an **empty result**, and concluded the anchor did not exist. The real commit was
`0b929ef` — verified with `git show --name-only`, which lists the file, and
`git rev-list --parents -n1` returns **3 entries, so it is a MERGE commit.** `git log -S` traverses
only the first parent by default and therefore **cannot see a change that arrived through a merge.**

**The zero was structurally guaranteed, not observed.** Use `git show --name-only <sha>` when you
have a candidate, and add `--full-history -m` (or check merges explicitly) when searching. This is
the sixth member of the instrument-manufactures-its-own-reading family in one session, alongside
`$?` after a pipe, ERE parens in a BRE context, `grep -c … || echo 0` emitting `"0\n0"`, a backtick
eaten inside single quotes, and the same command name resolving to different binaries.

**AN EXCEPTION LIST IS EVIDENCE TOO, AND THIS ONE WAS WRONG.** The same pass found that a
`FIXED_POINTER_ALLOWANCE` row — an entry whose whole purpose is to record where a fix landed —
**cited a commit that does not touch the file it claimed.** The exception list carried an unverified
evidence pointer: the identical defect, one level up from the one it was written to record. **Audit
your allowlists with the same probe you audit the code with**, or the list becomes the place wrong
evidence hides from the gate that would have caught it.

**AND A SUBAGENT FABRICATED A COMMIT SHA.** Four verdicts were attributed to `4aaae09`;
`git cat-file -e 4aaae09` reports **the object does not exist.** A second verifier attributed two
findings to a commit touching neither file. **A cited sha is a claim, and `git cat-file -e` is one
command.** Parallel verification still earned its cost here — as a *decoy detector*, not as
corroboration.


**AND IT DOWNGRADED THE INVESTIGATOR'S OWN BEST EVIDENCE, which is why this rule is worth more than
the correction.** `oracle_skew=0` was reported as two independent authorities agreeing about a
store. It is **two HTTP routes on the same daemon process, authenticated with the same token,
reading the same in-process state.** It proves one daemon is self-consistent; it does not
corroborate the store. A differential oracle whose two arms share a process, a credential and a
cache is not a differential oracle — **it is one reading, taken twice.**

The consequence for consumers ran the *other* way from what was announced: every CLI-derived figure
came from the DAEMON and is therefore MORE trustworthy than the fleet was told, not less. **The
numbers were fine; the explanation of where they came from was not.**

---

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

- Nothing here is installed. The binary does not exist.
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


The close policy REFUSES a prose reason, and the refusal scrolls past in-pane while the
agent moves on believing the close landed. Always read the status back:
`br show <id> --json | jq -r '.[0].status'` — note `br show` returns a BARE list, while
`br list` wraps its rows in `.issues`.

---

## Three graph and evidence rules that strangled real work tonight

### An epic OWNS its leaves via parent-child — NEVER a `blocks` edge onto its own leaf

A `blocks` edge from an epic onto a leaf it owns is **circular by construction**: the epic gates
the leaf, so the leaf cannot start until the epic closes, and the epic cannot close until its
children finish. **13 of the first 30 unassigned open beads were strangled this way, four of them
P0.**

`br show` reads `open` and unassigned and looks perfectly claimable. **The authority is ATTEMPTING
the transition and reading the refusal text** — quote it when reporting a blocker:

```
br update cp-u9ikt --status in_progress
  -> Error: cannot claim blocked issue: cp-epic-fleet-work-quality-08l6.74
```

**Find the writer before fixing the edges.** `br dep add <child> <parent>` **transposed** produces
exactly this shape, so repaired edges regrow while the writer still runs. Also: `br dep list <id>`
returns **OUT-edges only** — absence of an in-edge is not evidence of an orphan. And for triage use
`.triage.recommendations`, never `.quick_ref.top_picks` (it reports `unblocks=0` and omits
high-scoring beads); **skip epic containers**, whose PageRank accumulates from every child so they
top the list and can never close.

### A port that deletes a file invalidates every CLOSED bead that cited it

`45c613d` deleted four scripts. All four were legitimately superseded and every citing bead was
**validly closed at the time**. Hours later it surfaced as `check.sh` close-evidence RED with
everything downstream UNRUN — a gate refusing every dispatch, far from the mistake.

**Before `git rm`, grep CLOSED beads for the path.** A closed bead's evidence is a live dependency
on the filesystem, not a historical note. Measured exposure: 2 beads via `close_reason`, plus
comment-level citations the raw count hides. Tracked as `cp-rjuzj`; the commit-time gate that would
have caught it at the point of the mistake is `omp-orchestrator-pre-delete-citation-check-igk`.

### READ THE CONSUMER BEFORE SCANNING FOR IT — and the harvester manufactures its own failures

> ## ⚠ CORRECTED 2026-09-07 — THIS ENTIRE SECTION IS ABOUT **CONTROL-PLANE**, NOT THIS REPO
>
> **`crates/close-evidence-gate` DOES NOT EXIST HERE AND NEVER DID.** Measured:
> `git ls-tree -r HEAD --name-only | grep -c close-evidence-gate` → **0**;
> `git log --all --diff-filter=D --name-only -- 'crates/close-evidence-gate/*'` → **0**, so it was
> never deleted either; `grep -rn CITED_PATH crates/` → **0 occurrences anywhere in this repo**.
>
> It lives at `/Users/josh/Developer/control-plane/crates/close-evidence-gate/src/blob.rs`. **The
> readings below are REAL — of the wrong repository.** They were published here as "this repo's
> close-evidence extractor" by the same agent that wrote the fifth rule about not confusing the two
> boundaries. **A BORROWED CLAIM INHERITS ITS AUTHOR'S BURDEN applies to a borrowed REPOSITORY too**,
> and the tell was in the text the whole time: the tracking bead is `cp-…`, control-plane's prefix.
>
> **THE OPERATIONAL HARM IS THE IMPERATIVE, NOT THE CITATIONS.** *"Write every path in a bead
> comment inside backticks"* was published as governing this repo. **There is no harvester here to
> evade.** Agents were instructed to obscure paths from a consumer that does not exist — and this
> repo's real consumer runs the other way: `%19` measured 14 closed beads citing a `bin/` or
> `.flywheel/` path ONLY in comments, one of them the very bead that created the citation gate, and
> `crates/no-shell-gate/src/bin/pre-commit-gate.rs:342` now reads those comments through
> `read_closed_beads_from_mirror`. **Here, a backticked path is a path the gate should still see.**
>
> **Retained deliberately, because the mechanism transfers even though the location does not:** read
> the consumer before scanning for it, strip comments and fenced code before matching, over-strip
> rather than under-strip, and never size an extractor from an inferred regex. Those are why the
> section stays instead of being deleted. **Do not act on its paths, counts, or the backtick rule
> inside this repository.**

The close-evidence extractor was twice sized from an **inferred** regex. Read from source
(`control-plane:crates/close-evidence-gate/src/blob.rs:59` — **not this repo**) it is:

```
const CITED_PATH: &str = r"(?:^|[^\w/.])(bin/[\w.-]+|\.flywheel/[\w./-]+)";
```

Three facts that only reading it establishes:

1. **It harvests `bin/` and `.flywheel/` ONLY** — not `crates/`. A scan including `crates/`
   overstated the problem by ~13×.
2. **The gate reads `close_reason` + `comments`, NOT `description`** (`grade.rs:44-51`: the `Bead`
   struct has no description field). So a path in a description cannot break the gate — and a scan
   restricted to `close_reason` still **understates** it, because comments count.
3. **Fenced blocks and inline code are blanked before harvesting** (`blob.rs:95-96`:
   `fence.replace_all` then `inline_code.replace_all`). So in **control-plane**, backticks are a
   mitigation: a path written `` `bin/foo.sh` `` is invisible to that harvester.

> **THE BACKTICK RULE IS CONTROL-PLANE-ONLY AND IS RETRACTED FOR THIS REPO.** The `WAVE.md`
> measurement below was taken with control-plane's stripping applied to this repo's file, which is
> why it reported 0 — it measured a consumer that never reads here.

**And 47 of 71 unresolvable citations are a REGEX ARTIFACT, not broken evidence.** Both alternations
end in a greedy class containing `.`, so a sentence-ending period is absorbed:
`.flywheel/HARVEST-LOOP-PLAN.md.` — which resolves the moment the dot is stripped. A further 13 are
prose fragments (`bin/a`, `bin/b`, `bin/crates`, the last a truncation at the next slash). **The
real broken-citation count is 9**, and editing bead comments to work around the harvester would
leave it live to re-manufacture the same rows forever. Tracked as `cp-cited-path-trailing-period-n5mkc`.

## Post-mortem: the fleet went idle for 6+ hours while every watchdog fired (2026-08-31, session post-wave)

The session's product claim — no session goes idle until Joshua says so — failed for ~6 hours
(roughly 10:00Z to 16:00Z) while every detection layer worked. The causal chain, each link
measured, not inferred:

1. THE CONDUCTOR FAMILY IS WIRED AND FIRING — and refused at one gate, for hours. Cron entries
   exist for controller-tick (18,38,58), fast-dispatch (*/5), loop-driver, refill-idle-panes,
   challenge-lane, fleet-monitor, reap-finished-panes. controller-tick's log tail at 15:59:51Z:
   "ADMISSION REFUSED — no fresh standing PASS at check-sh-ledger.json". fast-dispatch's log:
   "drift UNRUN skipped-after-close-evidence; tests UNRUN; mutation UNRUN". The fail-fast chain
   means ONE red gate makes every downstream gate UNRUN, and the admission verdict can never go
   green while any single gate is red.
2. THE GATES WENT RED FASTER THAN THEY WERE FIXED. The standing check-sh verdict failed at 10:00
   (docs-staleness: the staleness metric counts commits since the doc's last DISK WRITE, so a wave
   committing ~2/min re-stales AGENTS.md in ~25 minutes). Fixed by landing real findings (5107abc).
   Then close-evidence RED: 39+5 closed beads without audit-trail comments — backfilled (34 fixed
   by close-reason evidence patterns already present; 5 unfixable by comments because
   close-evidence-gate's bead source omits the comments field entirely — source.rs:223). Then
   bead-lineage RED. Each fix revealed the next red: the chain re-fails on the next gate every
   time, and at wave rate the admission verdict was red ~continuously.
3. THE DISK WALL MADE THE REST OF THE CHAIN UNFIXABLE. The `tests` and `mutation` gates require
   cargo builds; builds are refused at the mint floor (container 6.5-6.8% vs 8%,
   CARGO_MINT_CONTAINER_EXHAUSTED exit 75) — escalated to Joshua (cp-oakbv). The admission verdict
   therefore cannot go PASS regardless of gate fixes until disk headroom exists.
4. THE WATCHDOGS DETECTED AND FILED — AND THE P0s SAT OPEN. challenge-lane auto-filed
   cp-rjuzj ("close-evidence RED blocks all dispatch") and cp-vgine ("idle OMP capacity beside a
   ready queue") — both P0, both correct, both sat open for hours. dispatcher-deadman exists for
   exactly this class. Detection fired; the response layer does not exist: every lane fail-closes
   on admission, and no mechanism is authorized to act on a DEGRADED signal.
5. THE CONDUCTOR WAS A PANE. Pane 1 hand-routed work all night (four grades, two fixes, the
   blocker map) — the manual orchestration was load-bearing while the automated conductor was
   admission-blocked. When pane 1 investigated the blockers, routing stopped and seven panes went
   idle. The product's own claim (loop-driver: single-instance deadline-bounded conductor;
   refill-idle-panes: "an idle worker beside a ready queue is the conductor's failure") is that
   the conductor is a BINARY. The binary exists, is cron'd, and was refused — see 1-3.

THE NAMED MECHANISMS THAT PREVENT RECURRENCE (in order of leverage):
  M1 — TYPED DEGRADED DISPATCH: when admission is red, the conductor dispatches LOW-STAKES beads
       (grading/verification/hygiene — the classes that need no green admission) with
       admission=stale marked on the lifecycle row. Challenge-lane's own acceptance says
       "dispatch the idle panes, OR name why the queue is not eligible" — the naming has run all
       night and must be allowed to end in a dispatch for the work that does not need a green
       tree. High-stakes dispatch keeps the full gate.
  M2 — GRADING AS A DISPATCH LANE: `crates/m2-grading-lane` consumes `dispatch_saga::m2::route`.
       Lineage is `--profile` from argv, not `AGENT_NAME`. Decide-only by default; `--apply`
       would send. It does not close. IMPL→GRADING *status* remains `dispatch-saga grading-transition`.


  M3 — CROSS-SESSION ROUTING: panes idled in a session whose repo was admission-blocked while
       real work existed in the other repo (grades, backfills, doc currency). The conductor must
       route by WORK LOCATION, not by session membership.
  M4 — THE DISK WALL (cp-oakbv, with Joshua): the admission chain's tests/mutation gates
       physically cannot run below the mint floor. Until resolved, M1 is the only dispatch path.
  M5 — DOCS-STALENESS METRIC REDESIGN: a counter that re-stales in 25 minutes on a wave is a
       gate that is red ~forever when the fleet is most active. Measure staleness against
       substantive-commit classes, or gate it to a longer window during declared waves.

ALSO CORRECTED IN THIS POST-MORTEM (the truncated-instrument class, third instance tonight):
an early crontab read (head -10) reported controller-tick REMOVED from cron; grep found it at
line 11+. Read the whole instrument. An uncommitted-edit attribution was also corrected by the
orchestrator to a committed-land state (228f42a) — check git status at report time, not from
memory.

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

# AGENTS.md — omp-orchestrator
> **SUMMARY:** This repository is the Rust extraction of the OMP orchestrator: typed ground truth,
> readiness, selection, dispatch, verification, and reaping. Read this file completely before acting;
> the lifecycle, cancellation, gate, and evidence rules below are binding.
**Session start:** read this file, `CLAUDE.md`, and `NEGATIVE_EVIDENCE.md` before acting; after compaction, re-read them before resuming.
The operating manual for any agent working this repo. `README.md` says what the product is and why.
This file says how you work here, what every crate is for, and what "done" means.

---

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
unwired. Four properties make it hold:

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
   general fix is to blank `//` and `/* */` before matching, exactly as `close-evidence-gate` blanks
   fenced and inline code before harvesting paths. **Over-stripping is the safe direction** — it can
   only report LESS reachability, and the failure this prevents is a false GREEN.
   The lesson is not about comments. **A mutation that fails to bite is the most valuable result
   available**: the suite was green, acceptance 4 was satisfied on paper, and only deleting the
   subject showed the gate could not see it.

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
5. **State the claim as a floor-raise.** Say what the gate mechanically enforces *and* what still
   passes. A residual "guarantees / proves / makes impossible" in a gate header is itself a defect —
   the overclaim is worse than the gap, because a reader stops looking.

6. **An `#[ignore]`d leg is not a passing leg.** Measured 2026-09-02:
   `findings_ledger::real_findings_ledger_is_strictly_valid` — the ONLY leg that validates the
   actual `FINDINGS.jsonl` rather than a fixture — is `#[ignore]`d, and `--include-ignored` has
   **zero callers** in this repo (the only matches are inside a vendored `asupersync` checkout
   under `.rch-tmp/`). It passes when run. So a default `cargo test` reports the gate green while
   production data goes unchecked: BUILT ≠ WIRED at test granularity. If a leg must be opt-in,
   something in-tree has to opt in.

7. **A known-bad leg must assert its MESSAGE, not just its exit code.** Measured 2026-09-02.
   `DJ-D-OT-UNGATED` rests on `cargo test -p no-shell-gate --test orchestration_tick` returning
   **101 / "no test target named `orchestration_tick`"**. During an unrelated fleet outage — a
   `Cargo.toml` under the `crates/*` glob with no `src/`, which breaks workspace **loading** so
   `-p <crate>` cannot dodge it — that identical command **also returned 101**, with a completely
   different message. **Same exit code, different cause.** Anyone re-running that Validation block
   mid-outage would have seen 101, ticked the box, and confirmed the finding on the wrong evidence.
   `101` is `cargo`'s generic failure; a fires-on-known-bad leg matching only `rc != 0` goes green
   on any unrelated breakage. Grep the specific string, and print the output so a reader can see
   which cause fired.

   **This is the sixth instrument defect of that session, and they are one family:** `$?` after a
   pipe returning the pipeline's status (three false findings, all in surfaces being audited FOR
   false success); a `timeout` ceiling below the subject's documented 5-minute deadline read as
   "unbounded" (three agents, one shared wrong model, each measuring their own SIGTERM); a
   `cargo metadata` probe whose `map(select($n|index(.)))` asked whether a list contained itself;
   a pane-identity probe whose input contained every agent name because the operator had typed them
   all; the flagship pre-commit gate exiting **0** on an empty index while the worktree was dirty at
   30 files. **In every case the instrument produced the reading, not the subject.** A gate's own
   evidence is subject to this too, which is why the message matters more than the code.

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

The close-evidence extractor was twice sized from an **inferred** regex. Read from source
(`crates/close-evidence-gate/src/blob.rs:59`) it is:

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
   `fence.replace_all` then `inline_code.replace_all`). So **backticks are the mitigation**: a path
   written `` `bin/foo.sh` `` is invisible to the harvester; written bare, it is harvested.

> **Write every path in a bead comment inside backticks.** Measured: this repo's `WAVE.md` harvests
> **0** paths under production stripping despite naming nine, because they are all backticked —
> while a bead body written in plain prose harvested **9**, five of which can never resolve.

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
  M2 — GRADING AS A DISPATCH LANE: beads in `grading` auto-route to eligible non-author graders.
       Tonight four grades were MANUAL orchestrator routings; the handoffs created waits that
       looked like idleness. (The close drought and the idle panes are one defect — confirmed
       again: 5cl/6gq sat in grading while panes idled.)
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
| dispatch | raw `tmux send-keys` | **`ntm --robot-send`**, `refill-idle-panes`, `fast-dispatch`, `controller-tick`, `loop-driver` — all installed and cron-scheduled |
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

The same shape, three ways in one session: prose instead of beads (`crates/finding` exists, zero
callers); `br create` instead of `Finding` (the standard exists as a type, unenforced); hand-grep
instead of `tick-monitor` (the census exists, unqueried).

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
when Bash calls reach the threshold with zero recorded hook invocations. COVERED, PARTIAL, and
UNKNOWN remain distinct outcomes. Shadow ledger evidence is diagnostic, not hook certification.

The installed rch-lane-bind binary at /Users/josh/.local/bin/rch-lane-bind is outside this repo.
Its stdin probe cannot prove that the Claude PreToolUse table intercepted a live call, and this
repo does not claim fresh-session coverage from a stale session. The measured wrapper limitation
is named rather than hidden: timeout, nice, env, time, stdbuf, and xargs wrappers may bypass a
resolver that inspects only the command-position token. Until a real intercepted probe closes
that class, wrapped invocations remain UNMEASURED/UNSAFE for coverage claims.

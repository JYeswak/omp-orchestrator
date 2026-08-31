# WAVE.md — who is who, who owns what, and how we talk

The live coordination contract for the `omp-orchestrator` NTM session. Read `AGENTS.md` first —
this file says only who does what and how you communicate.

---

## Roster — re-measured 2026-08-31 07:05Z

Keyed on `pane_id`, which is the only stable handle. The `Idx` column is a convenience that
**goes stale** — it already did: the previous version of this table listed `%1398`/`%1399`, which
exist in no live session, and inverted the codex/glm profiles. That cost a real lost handoff
(see below). Re-measured with:

```bash
tmux list-panes -a -F '#{pane_id} #{session_name}:#{window_index}.#{pane_index} #{pane_title}'
```

| tmux id | Idx | Agent | Model | Mail identity | Role |
|---|---:|---|---|---|---|
| `%1396` | 0 | — | shell | — | Operator scratch. Not an agent. Never dispatch here. |
| `%1397` | 1 | `omp-claude` | Opus 5 (anthropic, OAuth) | — (registration blocked, `-ygc`) | **Orchestrator / integrator.** Cross-crate decisions, verification, the only closer. |
| `%1413` | 2 | `omp-codex` | GPT-5.6-Luna (OAuth) | `GreenFrog` | **Extraction owner.** |
| `%1414` | 3 | `omp-codex` | GPT-5.6-Luna (OAuth) | `BlueLantern` | **Safety + conformance owner.** |
| `%1408` | 4 | `omp-glm` | GLM 5.3 (openrouter, 1.3M) | `AmberGate` | **Gate owner.** |
| `%1409` | 5 | `omp-glm` | GLM 5.3 (openrouter, 1.3M) | `GoldLark` | **Portability owner.** |

**Roster names are not pane ids, and the mapping is not guessable.** `GoldLark` is the mail
identity reserving `-7ai`'s files; the prose roster calls the same worker `SilverWolf`. Treat the
mail identity as authoritative for reservations and the `pane_id` as authoritative for delivery.

**The measured cost of trusting a stale roster:** at 06:58Z the gate owner closed `-4ak` and sent
its "gate landed, extraction unblocked" handoff to **`%1398`** — read straight out of the old table
above. `%1398` does not exist, so `GreenFrog` (`%1413`), the actual extraction owner, never received
it. A send to a dead pane id is silent. The orchestrator re-delivered it at 07:09Z, corrected,
and verified receipt at the receiving end. Tracked as `omp-orchestrator-75l`.

---

## Assignments

Each pane owns **one bead** and the files that bead names. No two panes hold the same file.

| Pane | Bead | Status 07:10Z | Owns |
|---|---|---|---|
| `%1408` | `omp-orchestrator-4ak` | **CLOSED** `3f821d4` | The no-shell/no-python gate. Landed first, on an empty tree. |
| `%1409` | `omp-orchestrator-7ai` | in_progress | Kill every hardcoded path. 3 repo-root + 6 home refs, measured. |
| `%1414` | `omp-orchestrator-5cl` + `-a3p` | `-5cl` in_progress, `-a3p` actionable | Forbid unsafe on all 23; then the unwired-lane conformance test. |
| `%1413` | `omp-orchestrator-815` | blocked on `-7ai` only | Extract 23 crates, deps-first. |
| `%1397` | `omp-orchestrator-kxe` | blocked on `-815` | The lifecycle binary. Integrates the rest; **starts last on purpose.** |

**`-815`'s stated precondition "commit the port in control-plane first" is CLEARED** and must not
be re-derived: all 12 files of the three ported crates are committed at `45c613d`, plus `8fc3e4b`
for the unsafe lint. What still blocks `-815` is `-7ai` alone — `GoldLark` holds four uncommitted
`src/*.rs` files in those crates, and copying them now would either take a dirty tree or silently
drop `-7ai`'s work.

**Ordering is not advice.** The gate lands before any crate is copied, or the repo starts dirty and
the gate gets weakened to make the build pass. Portability lands before extraction, or the new repo
inherits `/Users/josh/Developer/control-plane` — which **compiles** after a move and then silently
reads the wrong repo.

---

## Comms

**NTM is the notification layer. Agent Mail is not.** Mail's two real jobs are file reservations and
the durable record. Using mail to page someone makes coordination depend on an agent *choosing* to
check an inbox — a hope, not a mechanism.

| Question | Surface |
|---|---|
| "here is your next unit of work" | `ntm --robot-send=omp-orchestrator --panes=N --msg='<packet>'` |
| "I am about to edit these paths" | Agent Mail **reservation** — mandatory before shared-file edits |
| "the durable record of what I decided and why" | Agent Mail thread + a bead comment |
| "are you done / are you stuck" | `ntm --robot-wait` / a two-capture read — **never a mail poll** |

Agent Mail is healthy as of 2026-08-31 (`127.0.0.1:8765/health` → `status: ready`).

**Reserve before editing.** `file_reservation_paths(project_key="/Users/josh/Developer/omp-orchestrator",
agent_name=<yours>, paths=[...], reason="<bead-id>")`. Reserve the **narrow** set you will actually
edit — never `**/*`, which serialises the whole wave. Release when done.

**Thread on the bead id.** `thread_id="omp-orchestrator-4ak"`, subject prefixed the same way. One
thread per bead means the next agent can reconstruct the decision without asking.

**Delivery is not receipt.** `--robot-send` reports success when the text lands in a composer, not
when it is read. Verify at the **receiving** end by capturing the target pane, and sweep **all**
panes with a positive control — a packet delivered to the wrong pane looks identical to one
delivered nowhere.

**A pane is idle only on two captures.** `Working (27s)` and a frozen pane render identically.
Compare the timer **and** a spinner-stripped content hash ≥75s apart, reading the **last** status
line — a stale spinner sits in scrollback and will tell you a dead pane is alive.

---

## Standing rules for every pane

**Consult `fh` before you build.** Not once — at every non-obvious decision.

```bash
fh suggest "<the thing you are about to write>"
fh why <row-id>          # provenance before you believe it
```

`fh` already answered two questions in this wave: `beads_rust`'s `canonical_source_repo(beads_dir)`
is the repo-discovery pattern for `-7ai`, and `franken_lean`'s `UNWIRED_LANE_ALLOWANCE: &[] ` (an
**empty** allowlist) is the mechanism for `-a3p`. Reuse both; do not re-derive them. `fh` prints a
`STALE` banner when its ledger is old — read it and say so, because a row's age is part of the
citation.

**Search by meaning first.** `mcp__socraticode__codebase_search`, then `fh`. `grep` is the follow-up
that jumps to a line one of them surfaced, never the opening move.

**Every gate proves it bites.** Fires-on-known-bad **and** a known-good leg — an attack-only suite
ships an over-strict gate, and an over-strict gate gets routed around, which is a slower death than
no gate. Add a mutation that turns the leg RED, and restore byte-identically.

**Anti-vacuity.** An empty scan set is an **ERROR**, never a pass.

**Re-run, don't read.** A test that passed in CI yesterday is inadmissible. A test passes
meaningfully only when it (a) exists, (b) exits 0, **and** (c) asserts non-trivially against the
production path.

**Close reasons start with** `MUTATION-VERIFIED` / `DONE` / `APPROVED` / `WONTFIX`, or policy
refuses them and the refusal scrolls past unnoticed. Read the status back to confirm.

**Commits are path-scoped:** `git commit -- <explicit paths>`. Never `-A`. Messages carry a
verification-level tag.

**`br` reads the cwd.** `cd /Users/josh/Developer/omp-orchestrator` before any bead command, or you
will file into control-plane's tracker — which is exactly what happened once already.

---

## Known state, so nobody re-derives it

- **The port already happened.** `omp-idle-dispatch` (1033 LOC), `wired-but-inert-guard` (984),
  `fleet-composite` (1047) exist in control-plane, build clean, **29 tests pass**. The four
  `.sh`/`.py` originals are deleted. **It is UNCOMMITTED** — it must be committed in control-plane
  before extraction touches it.
- **Extraction set is 23 crates, not 20.** The three above are new.
- **Unsafe tally is 3 of 23**, not 2 of 20. `omp-idle-dispatch` already forbids it;
  `wired-but-inert-guard` and `fleet-composite` do not.
- **`cargo test` emits `error: unclosed table, expected ]`** pointing at asupersync's
  `tests/fixtures/migration_readiness_planner/malformed/Cargo.toml`. That is a **deliberate
  known-bad fixture in a vendored dependency**, not our break. Do not chase it.

---

## Tracker-query traps — measured, and each produced a CONFIDENT ZERO

Three defects on 2026-08-31 had one shape: **a query returned zero, and the zero was read as
"the data is absent" when the data existed and the query was wrong.** A confident zero is
indistinguishable from a healthy empty result, which is why all three survived review.

| Wrong query | What it actually returns | Right query |
|---|---|---|
| `br dep list <id>` to ask "what depends on this" | **OUT-edges only** — what `<id>` depends on | read the children, or `.triage.recommendations` |
| `bv --robot-triage \| jq '.triage.quick_ref.top_picks[]'` | a WEAKER view: `unblocks=0` on everything, omits high scorers | `jq -r '.triage.recommendations[:8][] \| "\(.id) score=\(.score)"'` — rank by `score`; `unblocks` is not even a key there |
| `git log --all --grep=<bead-short-id>` | matches the **tracker** commit (`chore(beads): consolidate…`) for unrelated ids | join on a SHA cited in the bead and verify it with `git cat-file -e` |

**`br show` returns a BARE list; `br list` wraps rows in `.issues`.** Guessing a jq path across
the two produces confidently wrong readings.

> **The generalisation, kept verbatim because two agents reached it independently within one
> hour (recorded in control-plane `cp-khil9`): "envelope field, not substrate."** Check the
> other fields of an envelope before diagnosing the tool.

**MATERIALIZED_ORPHAN = 0 as of 10:55Z.** An earlier note said 3 (`-gfb`, `-ygc`, `-6gq`). That
figure was produced by the out-edge bug above: `-gfb` had an in-edge from `-kxe` the whole time.
`-ygc` is closed; `-6gq` now carries a `related` edge to `-815`. **Do not re-derive 3 from a
stale note** — an orphan requires zero edges in BOTH directions, and `crates/tick-monitor`'s
`lifecycle` subcommand now computes in-degree in a second pass:

```bash
tick-monitor lifecycle --repo <repo> [--repo <repo>]
```

**The DAG is NOT edgeless.** Measured on the control-plane tracker: 1412 of 1864 records carry
dependency rows (1287 parent-child, 173 blocks, 5 discovered-from, 2 related). The real shape is
~7.4:1 **taxonomy over sequencing** — beads filed into an epic and never sequenced against each
other. So `unblocks=0` is TRUE and is an authoring gap, not a `bv` defect.

**Parent-epic inversion.** `br create --deps blocks:X` means *this bead depends on X*, which is
the inverse of what it reads like. A parent epic blocking its own child makes BOTH permanently
unclosable. `br create` does not show you the edges it made — **`br dep list` readback is the
only thing that catches it**, and `--force` with a recorded reason is correct where the graph is
already inverted.

---

## Build-lane and pane-classification traps — measured 2026-08-31

### The shared cargo target dir is caller-scoped, and a wrong lane COMPILES

`/Volumes/BuildShared/cargo-targets` is the resolved `target_directory` for **both** repos, and
**neither repo pins one**: `grep -A2 '^\[build\]' <repo>/.cargo/config.toml` finds nothing in
either. The lane comes from `FRANKEN_CARGO_LANE` in the **caller's environment**, so it is a
property of whoever invoked cargo, not of the repo.

**Four crate names exist in both repos** — measured by set intersection, not recall:

```bash
comm -12 <(ls omp-orchestrator/crates | sort) <(ls control-plane/crates | sort)
#   composer-typed  fleet-composite  loop-queue-filter  pane-dispatch-fence
```

> **Why it is worse than an ambiguity: `crates/fleet-composite/src/main.rs` is BYTE-IDENTICAL
> across the two repos (`diff -q` silent).** A build against the wrong lane compiles and passes.
> There is no error to notice. So *"I ran `cargo test -p fleet-composite` and it passed"* does
> not say **which repo's** crate was tested — and these four are precisely the extraction set
> mid-move, the moment duplication is most likely and least visible.

**Remedy:** pin `target-dir` per repo in `.cargo/config.toml` so the lane belongs to the repo.
**The value MUST name a target directory, never a lane root** — pointing it at a root makes
cargo create `<root>/debug` and `<root>/release` directly, and those are orphans by construction
that the reaper must never treat as lanes. That mistake has already been made here once.

**Status: a live mechanism with NO measured victim.** Only one lane currently holds a binary for
a shared name (`session-control-plane/release/fleet-composite`, 08-31 00:30). Filed as
control-plane `cp-fg5up`. It *threatens* banked verifications; it does **not** establish that any
are wrong. Do not upgrade that claim without a measured collision.

**Count discipline, learned the hard way:** an earlier note here said *seven* shared names. That
figure came from a recalled crate list, not an intersection — and it was stale twice over, since
`loop-coverage` and `ntm-fleet-monitor` had been removed from `omp-orchestrator` in the interim.
**A number that cannot be re-derived by a stated command is not a measurement.**

### A dialog-covered status line reads as a dead pane

A pane with an Ask/approval dialog open has its **model-name status line covered**, so a
classifier keyed on that line sees nothing and can score the pane GONE. Measured 2026-08-31
09:15Z: the fleet watcher scored `%1413` GONE when it had merely opened an Ask dialog.

| Reading | Means | Distinguish by |
|---|---|---|
| absent from `tmux list-panes` | **dead** | the pane id is not in the list at all |
| in the list, no model-name line | **alive and prompting** | capture deeper; look for the dialog |

Never map "status line not found" to dead. `crates/tick-monitor` has this bug too: its
`last_status_line` would return the dialog frame, and `classify` would score `Unproven` — which
is at least fail-safe (excluded from capacity) but is still the wrong reason.

### "Every 'wired' claim in this fleet is a source claim wearing a live badge"

Until an install path exists that ties a source commit to a running artifact, a green suite plus
a landed commit proves the **source**. The live lane is a separate artifact with its own
identity. Three cases in one session, each caught by hand: `arc-keepalive` (fixed `108476c`,
installed build still the old one), `controller-tick` (lifecycle landed `8802411`, installed
artifact 4 days older and missing all four lifecycle symbols), and `ntm-fleet-monitor` per its
own skill checkpoint. **Check the artifact, not the commit.**

---

## Graph and citation traps — measured 2026-08-31

### An epic must NEVER carry a `blocks` edge onto its own leaf

**The rule.** An epic **owns** its leaves through parent-child. A `blocks` edge from an epic onto
a leaf it owns is **circular by construction**: the epic gates the leaf, so the leaf cannot start
until the epic closes, and the epic cannot close until its children finish.

**It is systemic, not a one-off.** 13 of the first 30 unassigned open beads carry a `blocks` edge
from an epic, **including four P0s** (`cp-b56a6`, `cp-saegu`, `cp-u9ikt`, `cp-n56gj`).

**Why nobody sees it:** `br show` reads `open` and unassigned and looks perfectly claimable.
**Only ATTEMPTING the transition surfaces it:**

```
br update cp-u9ikt --status in_progress
  -> Error: cannot claim blocked issue: cp-epic-fleet-work-quality-08l6.74
```

**FIND THE WRITER BEFORE FIXING THE EDGES.** `br dep add <child> <parent>` **transposed** produces
exactly this shape, so 13 repaired edges regrow by morning if the writer is still running. Convert
**one** edge, prove the leaf becomes claimable, then do the rest — with `br dep cycles` clean
before and after and a recorded reason per edge. Tracked as `cp-0nfzp`.

### A port that deletes a file invalidates every closed bead that cited it

`check.sh` went RED on close-evidence with **everything downstream UNRUN**. Two of the seven
failures were ours: `cp-3k9jq` and `cp-op5uu` cite `bin/fleet-composite.py` and
`bin/omp-idle-dispatch.sh`, which tonight's shell-to-Rust port **deleted**. Both beads were
**validly closed at the time**; the port silently invalidated the citation, and it surfaced hours
later as a gate refusing every dispatch.

> **Before removing a file, grep CLOSED beads for its path.** A closed bead's evidence is a live
> dependency on the filesystem, not a historical note. Tracked as `cp-rjuzj`.

**MEASURED SCOPE, 2026-08-31.** `45c613d` deleted **four** files, and only **two** surfaced:

| deleted path | close_reason | description | comments |
|---|:-:|:-:|:-:|
| `bin/omp-idle-dispatch.sh` | **1** (`cp-op5uu`) | 2 | 3 |
| `bin/fleet-composite.py` | 0 | 2 | **4** (`cp-3k9jq`) |
| `bin/omp-idle-dispatch-selftest.sh` | 0 | 0 | 2 |
| `bin/wired-but-inert-guard.sh` | 0 | 1 | 2 |

Two consequences the raw count hides:

1. **`close_reason` is not the only field that breaks the gate.** `cp-3k9jq` has **zero** paths in
   its `close_reason` (104 chars, no path) and still failed — it cites `fleet-composite.py` in its
   description/comments. So a scan restricted to close reasons **understates** the exposure.
2. **Close-evidence evaluates a WINDOW of beads, not the tracker.** Fixing two and seeing green is
   a **false all-clear**: the rest are *unevaluated*, not passing. An unevaluated set reads exactly
   like a clean one.

**THE DURABLE FIX IS A PRE-DELETE CHECK, not a post-hoc repair.** Grepping closed beads for a path
*before* `git rm` converts a gate failure discovered hours later at dispatch time into a
**commit-time refusal at the point of the mistake**. Filed as `omp-orchestrator-pre-delete-citation-check`.

**AND A TRAP IN THE OBVIOUS GENERALISATION — measured, mine.** Scanning all bead-cited scripts
found **160** distinct `bin/*.sh|py` paths, of which **9** were absent from every repo working
tree. That is *not* 9 breakages: `git log --diff-filter=D` proves exactly **4** were ever present
and deleted (all in `45c613d`); the other 5 — `bin/x.sh`, `bin/jeff-issue.py`,
`bin/jeff-issue-rubric.py`, `bin/dcg-latency-bench.sh`, `bin/safety-config-write-gate.sh` — were
**never in any repo's history** and are foreign or illustrative paths. **"Absent from the tree" is
not "deleted from this repo."** Verify presence-then-absence, never absence alone.

One residue worth a look, independent of the port: `bin/safety-config-write-gate.sh` carries a
**close_reason** citation yet has never existed in any of the three repos — so a closed bead cites
a path that is either in a fourth repo or simply wrong.

### Adjacency is not authorship — a SHA near a bead is not a SHA belonging to it

`tick-monitor lifecycle` reported `omp-orchestrator-2lo landed=3f821d4`. That bead has **no commit
at all** (`git log --all --grep='2lo'` is empty); `3f821d4` is the `no-shell-gate` commit belonging
to `4ak`, and it appeared in 2lo's prose as a **citation**. So `LANDED_UNGRADED` inflates with
**phantom work**, and a phantom is indistinguishable from real work in the output because the SHA
resolves in git.

**The authoritative join is `git log --all --grep=<bead-short-id>` — the commit must NAME the
bead.** A SHA appearing only in prose belongs in a separate `cites=` field that never feeds the
queue. Until fixed (`…-phantom-landing-ma1`, blocked on `cp-oakbv`), treat `LANDED_UNGRADED` as an
**upper bound** and confirm each row before dispatching a grader at it.

Same tool, same failure, three times: `author=` reads the current **assignee** (observed live —
`r3h` flipped GreenFrog→AmberGate when AmberGate was routed as its *grader*), dangling citations
were counted as unresolvable commits, and now a resolvable SHA is credited to the wrong bead. It
kept narrowing *which tokens look like commits* and never asked **whose** commit one is.

### DERIVE, DO NOT QUOTE — your loaded copy of this repo's docs is a stale pin

The `AGENTS.md` loaded into an agent's context claims **20 crates, 25,567 LOC, 2-of-20 unsafe**.
The file **on disk** claims **24 crates, 32,087 LOC, 5-of-24**. Both are "AGENTS.md": the context
pins a snapshot taken at session start.

> An agent reasoning from its loaded context rather than re-reading the file will confidently
> restate **retired numbers in the tone of someone quoting project doctrine** — and cannot tell
> that its copy is old.

That is the mechanical cause of the 20→23→24 and 2→3→5 drift. **Re-read the file before quoting a
number from it.** Five confident-wrong queries in one session all looked like a healthy zero:
`br dep list` out-edges read as "no dependents"; `bv` `quick_ref.top_picks` null read as "no work";
`lsof | grep -c ESTABLISHED` counting **82** endpoint rows on a loopback port where the truth was
**41** connections; `path\s*=\s*"` matching `[lib]`/`[[bin]]` build targets as dependencies; and a
claimable-looking bead that only failed on the transition attempt. **Check what a pattern can ALSO
match, and check the other fields of an envelope, before diagnosing the tool.**

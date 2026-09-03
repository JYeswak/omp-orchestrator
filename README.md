# omp-orchestrator — a native Rust binary that runs a project end to end

> Install it on any of our machines, point it at a repo, and it drives that project from a plan
> to shipped, verified work using OMP agents — planning → beads → PageRank triage → dispatch →
> ground-truth verification → close. No shell. No Python. One binary.

## What this is

A single installable Rust binary that owns the **whole project lifecycle**, not one slice of it:

```
  plan  ──►  beads  ──►  bv triage  ──►  OMP agent panes  ──►  verify  ──►  close
 (planning-  (beads-     (beads-bv,      (dispatch with       (ground     (typed
  workflow)   workflow)   PageRank)       typed acceptance)     truth)      receipt)
```

Today that pipeline exists as **160 shell scripts and 60,467 lines** in one repo, plus 20 Rust
crates that already ported the hard parts. This project is the consolidation: the Rust becomes the
product, the shell does not come with it.

## Why we are building it

**1. The shell is not incidental — it is the majority of the operational surface, and it keeps
producing defects that Rust makes inexpressible.** Measured in a single lane on 2026-08-31:

| Defect | What happened | Inexpressible in Rust because |
|---|---|---|
| Backtick injection | A backtick inside a *bead body* was executed as a command. `msg` was never assigned, so every scheduled fire dispatched **nothing** while still logging healthy idle counts. | Bead text is a `String`, never a program |
| `mapfile` absent | cron resolves `env bash` to **bash 3.2**, where `mapfile` does not exist. Every hand-run test passed; the scheduled lane was dead. | One compiled binary, one runtime |
| Heredoc stole stdin | `cmd \| python3 - "$n" <<'PY'` — the heredoc claimed stdin so the piped JSON never arrived. Reported `idle=1 dispatched=0` **with no error**. | Typed function call, no stdin contention |

None of these were visible by reading the code. All three were found by watching a scheduled lane
fail silently.

**2. A lifecycle spread across 176 scripts cannot be installed anywhere else.** The value is the
*process* — plan → beads → triage → dispatch → verify — and a process you cannot install is a
process that lives in exactly one checkout on one machine.

**3. Typed state makes the honest answer cheap.** "Is this pane working?" and "did this bead
actually close?" are enum questions. In shell they are string-matching questions, and every string
match we shipped has been wrong at least once.

## Scope boundary

**control-plane is the proving ground.** This substrate is currently installed on the Studio for the
`omp-orchestrator` session only. It travels to another repo or machine only after that target has its
own live proof; source code, an installed binary, and a loaded launchd job are separate claims.

## Non-goals

- **Not** rebuilding NTM, FrankenTerm, Agent Mail, `br`, or `bv`. We wrap the existing stack.
- **Not** an unmanaged daemon or autonomous ringleader. The resident supervisor is a first-class
  bounded controller: launchd owns its lifetime, every cycle emits a durable heartbeat, and idle
  capacity requires dispatch, a typed escalation, or Josh's expiring authorization.
- **Not** a shell-to-Rust transliteration. Where the shell was wrong, the port fixes it and says so.

## Resident supervisor

The `omp-orchestrator-kxe` path is now a resident Rust binary. The checked-in launchd job is
`launchd/ai.zeststream.omp-orchestrator.plist`; it runs the installed binary with explicit `HOME`,
`PATH`, `TMUX_TMPDIR`, repository/session, worker-pane exclusions, and state paths.

Each cycle executes `tick-monitor observe` → `br ready --json` → typed decision → transport-specific
dispatch → receiver-side proof. `SupervisedWorking` writes a heartbeat and continues.
`QueueEmptyNeedsJosh` is a nonzero typed escalation. `IDLE_UNAUTHORIZED` is never silently consumed.
`--robot-send` is not a receipt: codex panes route through literal `tmux send-keys -l`, and the
shared receiver contract distinguishes idle-to-working, working-to-working, dialogs, absent panes,
and empty pane lists. An uncertain send leaves a durable pending-dispatch fence rather than retrying
the same bead.

The current Studio deployment has an explicit pending-dispatch fence for `omp-orchestrator-undrained-pipe-lint-w4j`
after the packet was visible in `%1413` but the first post-send observation was unavailable. That is
an honest no-claim, not a successful receiver proof; Josh must inspect or clear the marker.

## The cancellation contract (asupersync)

Every process this binary spawns — `tmux`, `ntm`, `br`, `bv`, a build — is cancellable work with a
deadline. It is built on **asupersync 0.4.9** (`/Volumes/ZestData/dicklesworthstone-mirror/asupersync`,
a real `[lib]`), and these are contract, not style:

- **`&Cx` first** in every async API we own; `cx.checkpoint()` in loops, retry bodies, and long
  handlers so cancellation is observable rather than hoped for.
- **Region-owned tasks** via `Cx::spawn` / `Scope` child regions. **No detached tasks** — a detached
  spawn is how a cancelled dispatch keeps writing to a pane that has moved on.
- **Kill the process GROUP, never the pid.** Measured: `child.kill()` signalled one pid while its
  grandchildren survived as orphans (`ppid=1`, 0.0% CPU) **still holding the admission lock**. Every
  timeout then guaranteed the next attempt failed too — the failure created the condition for its
  own repetition.
- **Drain the pipes.** Piping stdout *and* stderr then polling `try_wait()` deadlocks any child that
  writes past ~64 KiB (each stream has its own buffer). Measured: a `git log` that takes 0.9s from a
  shell sat at **0.0% CPU for 104s** as a child. The tell is 0% CPU with no children — a slow
  computation burns CPU, a deadlock does not, so widening the timeout makes it *worse*.
- **A timeout is not a verdict.** A killed child's empty stdout must map to `TIMEOUT`, never to the
  same token a genuinely failing subject produces. Measured: parsing an empty buffer for a `verdict`
  field and defaulting to `FAIL` manufactured a claim about the fleet out of nothing.

## The lifecycle, and which skill governs each stage

| Stage | Governing skill | What the binary does |
|---|---|---|
| Plan | `/planning-workflow` | Converge in plan-space first; plan-space is cheap, implementation-space is ~25× |
| Plan → beads | `/beads-workflow` | Every bead self-contained, with **testable acceptance: run X, expect Y** |
| Triage | `/beads-bv` | PageRank over the DAG; work the articulation points, not the comfortable leaves |
| Dispatch | `/vibing-with-ntm` | One bead, exact owned files, required proof, stop conditions — to a pane proven idle |
| Verify | `/beads-compliance-and-completion-verification` | **Re-run, don't read.** Status is a claim, not a fact |
| Close | typed receipt | Ground truth only: a commit, a bead close with cited evidence, a structured ack |

**The rule that binds all six:** a bead with no acceptance criteria cannot be worked, only
adjudicated — and adjudication reliably produces "no work to be done" instead of work. Measured: a
P0 bead at the head of the ready queue had **no ACCEPTANCE section at all**, and two agents in a
row triaged it and idled rather than shipping.

## How we reach OMP today — by scraping its terminal

These crates exist to orchestrate OMP. Measured on 2026-08-31 against the installed
`@oh-my-pi/pi-coding-agent` v18.0.11 (`~/.local/lib/node_modules/@oh-my-pi/pi-coding-agent`,
`dist/cli.js` is 19M), the surface OMP publishes and the surface we consume do not overlap at all:

| OMP publishes | We consume |
|---|---|
| **39 CLI subcommands** in the `omp --help` COMMANDS block — `acp`, `agents`, `auth-gateway`, `models`, `plugin`, `ps`, `read`, `search`, `share`, `shell`, `ssh`, `worktree`, … | nothing |
| **57 type-surface directories plus 14 top-level declaration files** under `dist/types`, 71 entries in total — `jsonrpc`, `tools`, `slash-commands`, `commands`, `session`, `task`, `goals`, `plan-mode`, `modes`, `subprocess`, `exec`, `dap`, `debug`, `capability`, `registry`, `extensibility`, `memories`, `irc`, `collab`, `live`, `telemetry-export`, `eval`, `security`, `sdk`, … | nothing |
| **`--mode=<text\|json\|rpc\|rpc-ui>`** as a documented top-level flag, so an RPC transport already exists | nothing |
| **3 `omp/*` methods** in the bundle: `omp/muxConnect`, `omp/muxPing`, `omp/muxRestartServer` | nothing |

Counted precisely, because the first count was wrong:

```
find dist/types -maxdepth 1 -mindepth 1 -type d        | wc -l   ->  57
find dist/types -maxdepth 1 -mindepth 1 -name '*.d.ts' | wc -l   ->  14
```

That number was first published as "71 directories" and a worker pane independently measured 57.
Two measurements of the same thing disagreed, and the reconciliation was that the first had counted
*entries* and called them *directories*. Neither count was fabricated; the **noun attached to it**
was. That is the same class of defect as every other confident-wrong figure in this file — the
arithmetic was never the problem.

Four greps across every crate's `src`, all zero: `Command::new("omp")` → **0 files**. `mode=rpc` →
**0**. `muxConnect` → **0**. `omp/` → **0**. What the crates actually spawn is `br` at 5 sites,
`git` at 4, `tmux` at 1, `cargo` at 1.

The orchestration channel is a terminal, and every classifier defect measured today is downstream
of that one fact:

- **Pane state comes from a braille spinner regex**, because there is no state RPC to ask.
- **A "receiver receipt" is a timer reset plus a spinner-stripped content hash**, because a
  `send-keys` has no delivery response to read.
- **Two codex panes read `<no marker>` today** because a tool-call box border renders *after* the
  status line. That artifact cannot exist over a typed protocol — it exists only because we are
  parsing a rendering.
- **`ntm --robot-send` refuses codex panes with "cod composer not visible"** (`cp-nq2s9`). That is
  a terminal-inspection guard, not a protocol error: nothing about the bead was wrong, the screen
  was.
- **Both polarities of transport failure have fired.** `cp-z42vu`: a send returned `success:[4]`
  while the packet never arrived. The inverse fired today in the pending-dispatch marker — the
  packet was visible, the confirming observation was not. That pair is the signature of one
  unacknowledged transport, not of two unrelated bugs.

**NO-CLAIM.** "No crate calls OMP" is measured for *our* crates only. NTM may itself speak an OMP
protocol beneath `--robot-send`; that is **unmeasured**. The evidence leans against it — a protocol
would not phrase a refusal as composer visibility, and would not need a timer reset to stand in for
a receipt — but leaning is not measuring, and this stays a no-claim until someone reads NTM's send
path.

Of the surfaces we do not consume, four are **reimplemented by scraping** — pane state, dispatch,
session, health check — and each has an OMP RPC or CLI alternative that exists today. Five are
**should-use**: `omp/muxConnect`, `omp/muxPing`, `omp/muxRestartServer`, `goals`, `collab`.

**NO-CLAIM.** Mapping a surface is not adopting it. Some scraping may be correct precisely because
no alternative exists for third-party panes OMP does not own. The gap between 57 published
type-surface directories and the zero we consume is this project's central open question; stating
it makes the choice visible, it does not force a rewrite.

## Hard gates (they fail the build, not a report)

1. **No `.sh`, no `.py`.** A Rust gate walks `git ls-files` and refuses either extension. It lands
   *before* the first crate is copied — a gate that arrives after the mess gets weakened to make the
   build pass. Planted known-bad both directions plus a mutation leg.
2. **`#![forbid(unsafe_code)]` in every crate.** Today **2 of 20** carry it. A crate that will not
   compile under the lint is a finding, not a reason to drop the lint.
3. **Every gate proven to bite.** A gate with no fires-on-known-bad is not evidence of anything; a
   gate with *only* attack legs is over-strict and gets routed around. Both directions, always, plus
   a mutation that turns the leg RED.
4. **Anti-vacuity.** An empty scan set is an **error**, never a pass. A deliverable that was never
   checked reports identically to one that passed.
5. **Every crate declares which OMP surface it maps to.** A crate that orchestrates OMP without
   naming the subcommand, type surface, or RPC method it stands in for is scraping by default, and
   the census flags it rather than letting the omission read as a design. Filed as
   `omp-orchestrator-omp-surface-map-41b` and **not yet built** — a stated gate, not yet one with a
   fires-on-known-bad leg.

## What we are building, in phases

The product is one binary that drives a project from an idea to shipped, verified work. That journey
is cut into **twelve boxes** — the phase arc — and each box is a contract with measurements, gaps, an
agreement ledger, and a rendered diagram before any of it is built. The boxes live in
`docs/plan/flow/boxes/` and the governing contract is `docs/plan/flow/CONTRACT.md`.

| box | what it is | status |
|---|---|---|
| **S1** | human start: install → system check → ecosystem check → walkthrough → swarm live → portal | **BUILDING** |
| S2 | idea → plan: prose sections become an assembled plan | draft |
| S3 | plan → grade: fresh-eyes rounds under a named grader | draft |
| S4 | plan → beads: a graded plan becomes a dependency DAG | draft |
| S5a | select: BV-ranked ready work, non-epic, unclaimed | draft |
| S5b | claim → dispatch: fenced packet render and tracker projection | draft |
| S6a | receipt ACK: independent post-observation carrying delivery evidence | draft |
| S6b | work in progress: tracker-visible stage, blockers, silence detection | draft |
| S6c | verify → grade → close: independent evidence, non-author grader | draft |
| S7 | validation: eight-gate pre-commit dispatcher | draft |
| S8 | install and rollback: identity-checked supervised install | draft |
| S9 | decision ledger: durable owner-routed halt and approval | draft |

**S1 is authorized to build; S2–S9 are frozen.** `gate-s1-djn8` blocks `gate-s2-ehx8`, so S2 cannot
start until S1 closes. The reason for that ordering, and the reason the freeze had to be *amended*
rather than simply lifted, is recorded in `CONTRACT.md` — the original exit condition was a cycle:
`converged → needs functionality → needs a crate → needs the freeze lifted → needs converged`.

### S1's six layers

S1 is not one step. It is the layer stack a new human walks through, and each layer has its own
contract in `docs/contracts/` and its own gate in the bead DAG:

```
L0 install  →  L1 doctor  →  L2 ecosystem  →  L3 walkthrough  →  L4 swarm live  →  L5 portal
   gate-s1-10-jtgw  -11-fnv8  -12-j5m9        -13-z8hz          -14-hs15         -15-w44h
```

Decided by the repo owner on 2026-09-03 and recorded in `docs/decisions.jsonl`: L3 ships a
**frankentui Rust TUI for humans plus `--json` for agents** over one step array (a divergence between
the two fails the build, HD-0009); the L4 spawn default is **1 claude + 2 codex + 1 grok**, read from
config and never hardcoded (HD-0010); Stop-hook strictness is **not authorized until it is testable
and measured** (HD-0011); **no hook is built before the hook surface is probed** (HD-0012); and darwin
binaries build on the **local Mac** while Linux/Rust compilation offloads to contabo — no macOS SDK is
placed on the Linux lane (HD-0013).

## The crates

**Do not cite a crate count from this file. Run the command** — this figure has moved four times
inside the lifetime of one document, and every stale value was corrected by an agent who then typed a
fresh integer that went stale in turn:

```bash
# these two must agree, or you have a manifest-vs-directory discrepancy
cargo metadata --no-deps --format-version 1 --offline \
  | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["packages"]))'
find crates -mindepth 1 -maxdepth 1 -type d | wc -l
```

One dated measurement, as evidence the commands run — never as a figure to cite. **2026-09-03: 72
and 72, agreeing.**

The crates group by the job they do in the loop. `AGENTS.md` carries the full per-crate table with
the source audit; this is the map.

| group | what it answers | crates |
|---|---|---|
| **ground truth** | what is actually true right now | `fleet-composite`, `pane-truth`, `fleet-truth`, `fleet-reconcile`, `oracle-compare` |
| **readiness** | may this pane receive work | `pane-dispatch-fence`, `composer-typed`, `pane-dispatch-ready` |
| **selection** | what should be worked next | `loop-queue-filter`, `refill-idle-panes`, `loop-coverage` |
| **dispatch** | send the work | `dispatch-claim-fence`, `dispatch-silence-watch`, `fast-dispatch`, `finding-dispatch` |
| **receipt** | did it arrive and was it read | `ack-spine`, `ack-stage`, `receiver-receipt`, `inbox-monitor` |
| **observation** | is the fleet alive | `tick-monitor`, `fleet-monitor`, `bead-availability`, `reap-finished-panes` |
| **evidence** | can a claim be believed | `lifecycle-event`, `finding`, `decision-ledger`, `convergence-stamp` |
| **gates** | refuse it at the point of the mistake | `no-shell-gate`, `commit-build-fence`, `staged-build-gate`, `path-literal-guard`, `state-wildcard-lint`, `undrained-pipe-lint`, `kernel-bypass-gate`, `crate-atom-gate`, `preregistration-gate`, `pre-delete-citation-check`, `orchestration-tick-gate`, `porting-gate` |
| **substrate** | the boundary primitives | `subprocess-contract`, `scratch-home`, `agent-mail-native`, `omp-rpc-session`, `omp-types`, `sender-identity` |
| **surface map** | which OMP surface each crate stands in for | `omp-inventory-map`, `omp-surface-consumption` |

**Every crate in that table is present in this workspace.** An earlier draft of this section starred
seven of them — `pane-truth`, `fleet-truth`, `fleet-reconcile`, `oracle-compare`,
`pane-dispatch-ready`, `refill-idle-panes`, `loop-coverage` — as "extraction targets, only source is
control-plane". **That was wrong and the stars are removed.** All seven are real crates that `cargo`
loads as workspace members, measured 2026-09-03:

```
fleet-reconcile      1432 LOC  2 test files      pane-dispatch-ready  2876 LOC  5 test files
fleet-truth          1619 LOC  3 test files      pane-truth           1499 LOC  3 test files
loop-coverage        1374 LOC  0 test files      refill-idle-panes    2908 LOC  1 test file
oracle-compare        473 LOC  0 test files
cargo metadata --no-deps -> all 7 loaded, 0 not loaded
```

The stars came from `AGENTS.md`'s crate table, whose `STATUS` column still reads `CONTROL-PLANE` for
those rows. **The extraction has landed and the table is stale** — which `AGENTS.md` itself predicts
in the same section: *"Read the STATUS column before you reason about any row… a historical
extraction-target list, not the package inventory."* The lesson is the one that keeps repeating here:
a status recorded in prose is wrong the moment work lands, so verify the row before repeating it. Two
of the seven (`loop-coverage`, `oracle-compare`) have **zero test files**, which is the more useful
finding the correction surfaced.

### Built this session

- **`lifecycle-event`** — the declared `LifecycleEvent` artifact had **zero instances on disk**
  (`SCHEMAS.toml:138` declared it; `FOUNDATION.jsonl`'s rows were hand-written). One type, **two
  carriers** — L0 runs before a repo exists and the installer has no `Cx`, so it needs a host sink
  while L1–L5 write into the repo. Six call sites, readback-enforced, `fsync` on file *and* parent
  directory, wired from four real callers.

### Building now

- **`s1-coverage`** — the readiness number must regenerate from a clean clone. Today it does not:
  the generator is an untracked `.py` under `.git/`, which also breaks the no-shell rule by
  *geography* rather than following it. Reads a git **tree** via `--rev`, not the worktree.
- **the observability monitor column** — one reader per layer (or one with a `--layer` flag, under
  argument), plus a carrier for the six metric thresholds that currently have **no home**:
  `NUMBERS.toml` cannot hold a duration, so every metric row is permanently `UNMEASURABLE`.
- **the hook design contract** — the omp hook surface is **TypeScript** (`extensions:` in the profile
  agent config, 9 factories), which collides with the doctrine that every live hook be a Rust binary.
  Three options and one must be chosen rather than defaulted into; see `S1-HOOK-OMP-PANES`.

### Roadmap, in the order the DAG forces

1. **Close S1's six layers.** Requirements are artifacts — an event writer, artifact+readback, a
   monitor, a gate with a known-bad leg, a metric, a named test, per layer. `S1-COVERAGE.md` is the
   burndown, and it is only trustworthy once `s1-coverage` makes it regenerable.
2. **Certify the hooks.** Twelve fields per row plus the six `/hook-certification` stages. Measured
   2026-09-03: **zero of 27 live hook instances** has entered those stages, and three live `.sh`
   hooks survive under `~/.claude/` and `~/.codex/` where `git ls-files` cannot see them.
3. **Stop scraping OMP.** Zero of the 39 subcommands, 71 type-surface entries, and `--mode=rpc`
   transport are called from any crate. Every pane verdict is a claim about a *rendering*.
4. **Extract the starred crates** from control-plane, leaves first.
5. **Then S2–S9**, gated behind S1.

**NO-CLAIM.** This section is a map, not a certificate. `MISSING` is large, the coverage generator is
not yet regenerable, and a crate appearing in the table above is not a claim that it is wired — the
`BUILT ≠ WIRED` rule applies to every row, and `AGENTS.md` records that 30 of 68 crates had no caller
when last measured.

## Status

Two things are true at once.

**The resident supervisor is deployed and launchd-owned.** launchd owns `omp-orchestrator` on the
Studio, the installed binary reports its build identity with `--version`, and the live heartbeat and
supervisor logs are under `~/.local/state/flywheel/`.

**The binary does not speak OMP's protocol.** It types into panes and reads status lines. Zero of
the 39 subcommands, 57 type-surface directories, and `--mode=rpc` transport OMP publishes are
called from any crate — see *How we reach OMP today*. Every pane verdict this binary emits is
therefore a claim about a rendering, not a claim from OMP.

The current deployment carries a pending-dispatch fence after one packet was visible in `%1413` but
the post-send observation was unavailable; this is intentionally not claimed as an end-to-end
receiver proof. The repository remains a proving ground, not a universal deployment.

# 08 — The end-user journey: another repo, another machine, orchestrating their own project

Serves **R9** — *"all the way to end users (other projects / repos / machines) are using it to
orchestrate their projects."*

**Read the status marker on every claim here.** Almost nothing in this section exists. The adoption
path is `PROJECTED` end to end; the measurements are of *our own repo today* — evidence the failures
are real, and the exact obstacles between us and a foreign adopter.

`MEASURED` 2026-08-31, and it frames everything below: there is no adopter-facing entry point at all.
`/usr/bin/grep -cE '"doctor"|"init"' crates/omp-orchestrator/src/main.rs` → `0`. The whole CLI
surface, verbatim from `crates/omp-orchestrator/src/main.rs:254`:

```
usage: omp-orchestrator [--once|--max-ticks N] [--repo PATH] [--session NAME]
                        [--interval-secs N] [--receiver-agent NAME] [--omp-quick] [--omp-binary PATH]
```

No `doctor`, no `init`, no `adopt`. `--session NAME` and `--receiver-agent NAME` are facts about
*our* fleet a stranger cannot supply correctly. **NO-CLAIM:** that grep establishes the absence of two
literal strings in one file, not that no other crate offers an entry point (§07 owns distribution).

---

## 1. Who the end user is

**Persona A — the solo maintainer.** One repo, one machine, no fleet, 5k–100k lines. One agent at a
time in one terminal, by hand; no panes to census, no use for a supervisor loop. They want the *back
half*: gates that fail a build on a named property, and completion tracking where "done" means an
artifact exists rather than an agent said so. They can be served first, because the gates are the
part of this repo measured to work — `MEASURED` (brief §3.5, recomputed): 26 integration test files,
379 `#[test]` functions, 8 gate crates, **2 of 8** with all four legs (`no-shell-gate` 4/3/2/6 and
`undrained-pipe-lint` 1/1/1/3), **4 of 8** with no mutation leg. **NO-CLAIM:** that leg table counts
files matching a property grep; it does not establish the legs are individually strong.

**Persona B — the small team.** Three to eight agents, one repo, one shared session. They already
dispatch by hand and already lose track. They want dispatch with receipts: a record that a packet
was accepted, by whom, and whether work started. `MEASURED` (brief §4): the `actuate` layer **does
not exist** — a human types into panes; the `complete` layer **does not exist** — every completion
this session was found by a human looking. Their core need is the two empty rows of that table.

**Persona C — the multi-repo fleet operator.** Our own shape: many repos, dozens of panes, a tracker,
a queue, a supervisor loop. Last to serve, structurally: `MEASURED` (brief §3.2), 157 of 183 rows are
`CAPABILITY_NOT_USED` and all 7 `consumes` edges come from one crate — an inventory, not orchestration.

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
`/Users/josh`; `:12` hardcodes three binary names. Repo-wide the pattern appears **16** times across
**14** files (harness grep for `CARGO_MANIFEST_DIR` under `crates/`; a shell `grep -r --include=`
returns a false zero here — see §2.2). An installed binary carrying a compile-time path audits the
*build machine's* checkout, not the adopter's. **NO-CLAIM:** most of the 14 are tests, deliberately.

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

`MEASURED` grounding for the first refusal: `crates/tick-monitor/src/lib.rs:485` sets
`MIN_GAP_SECS = 75` — liveness is a two-capture property one tick cannot prove, so `UNPROVEN` on tick
one is correct and must be labelled or it reads as a bug. The `receipt` object has a measured shape:
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
| our directory layout (`crates/`, `docs/plan/`) | `MEASURED`: 14 files resolve roots from `env!("CARGO_MANIFEST_DIR")` (§2.1) — a workspace assumption, not a universal one | `RepoAdapter::root()` resolves at **runtime** from cwd upward; compile-time roots stay in our own tests |
| our tmux session naming (`--session NAME`) | required today (`crates/omp-orchestrator/src/main.rs:254`), encoding our fleet's shape | `WorkerAdapter` owns naming; `--session` becomes a tmux-adapter-scoped flag, invalid elsewhere |
| **our `.sh`/`.py` prohibition** | OUR accretion rule, born of a measured 160 tracked shell scripts and 60,467 lines in `control-plane` (`crates/no-shell-gate/src/lib.rs:6-9`). A foreign repo full of shell scripts is a normal repo and must be fully orchestrable | `no-shell-gate` is **opt-in**: `SKIPPED reason=OPTED_OUT_BY_ADOPTER`, never in a default set |
| our specific agent CLI (OMP v18) | §3.1 — the deepest coupling in the codebase | `WorkerAdapter::observe()`; `tick-monitor` becomes the *OMP-v18 implementation*, not the interface |
| a single version flag that works on every dependency | `MEASURED` by me without a pipeline: `tmux --version >o 2>e; echo $?` → **exit 1**, 0 bytes stdout, 158 bytes stderr; `tmux -V` → `tmux 3.6a`, exit 0. tmux is **well-behaved** — an earlier claim in this batch that it "exits 0 while failing" was an artifact of reading `$?` after a pipeline, where the status belongs to the last command. `--version` answers 8/9 of our binaries, `-V` 5/9 | `doctor` requires **two independent presence signals** and pins **each arm with its own test, including the failure arm**. Precedent, verified first-hand in `pi_agent_rust/src/doctor.rs` and cited by construct because a bare line number is unverifiable: `:950` the naive success arm, `:967-968` the two-signal arm (`discovered_path.is_some() && probe_failure_is_known_nonfatal(…)`), `:1066` `which_tool` as the independent signal, `:13948` `check_tool_falls_back_when_probe_args_are_unsupported`, `:13964` `check_tool_reports_invocation_failure_for_broken_executable` — the second test is the known-good leg that stops the fallback becoming a blanket amnesty. **ADOPT WITH A NAMED GAP:** `probe_failure_is_known_nonfatal` at `:1057` allowlists exactly one tool (`if tool.ne("sh") \|\| args.ne(&["--version"])`), so a doctor built on that code marks tmux MISSING today |

We may enforce our own rules on ourselves as hard as we like. `MEASURED`:
`git ls-files -- '*.sh' '*.py' | wc -l` → `0` (grep-free, deliberately), exemption list empty by
design (`crates/no-shell-gate/src/lib.rs:6`). **Exporting it would be colonisation** — and it misfires
even on us: `crates/composer-typed/tests/differential.rs:41` aims its oracle at
`../../bin/composer-typed.py`; `ls bin/` → `No such file or directory`. **NO-CLAIM:** index only.

### 3.1 The deepest coupling, stated as the objection an investor should raise

*"Your observer reads one vendor's terminal UI. You have not built an orchestrator, you have built an
OMP v18 screen-scraper. What is the adapter story worth if the layer you claim works is vendor-blind?"*

Correct as stated, and the strongest objection here. `MEASURED` and specific:
`crates/tick-monitor/src/lib.rs:312` hardcodes `MODEL_MARKERS = ["Opus 5","GLM 5.3","GPT-5.6",
"GPT-5.5"]`; `:315` hardcodes three OMP-v18 dialog-footer strings *captured verbatim from pane
`%1372`*; `:383` strips braille `U+2800..U+28FF` and the literal `π`; `classify` matches two verbatim
queued-message strings (harness grep, `capture\.contains` → 2 sites). 1,185 lines, one vendor.

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
(`crates/tick-monitor/src/lib.rs:485`). References, `MEASURED` as existing:
`crates/tick-monitor/src/lib.rs` (`PaneState`, `Liveness`, `Observation`, `classify`) is the
*tmux + OMP v18* `observe`; `crates/receiver-receipt/src/lib.rs` (`ReceiptVerdict`,
`assess_receiver_receipt`) is `receipt`; `crates/ack-stage/src/lib.rs` is `dispatch`.

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

*A supervisor running 23 commits stale.* `MEASURED`, session ledger, 2026-08-31. **Provenance defect,
recorded rather than hidden:** I found no re-derivable command for the `23`, which writing-contract
rule 1 forbids; the failure class is real and the missing command is a defect against this document.
Prevented by applying `crates/installer/src/lib.rs`'s identity proof to the *running* supervisor.

*162 refused ticks with nobody watching.* `MEASURED` (brief §4): 162 refusals over 4.2 hours,
`DISPATCH_RETRY_BLOCKED`. The fence was right; the silence was wrong. Prevented by a refusal budget —
N consecutive refusals of one code escalates, because silent perpetual refusal reads as a hang.

*A gate that is correct and unreachable.* `MEASURED` (brief §3.6): `omp-inventory-map --help` →
`CONFIG_ERROR unknown argument --help` on a gate whose 13 tests pass. Prevented by **ADDRESSABLE**.

`MEASURED` denominator: the board at stand-down was 28 closed, 25 in_progress, 19 open, 2 blocked.
**NO-CLAIM:** each item pairs a measured failure with an intended mechanism; none has been observed.
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
more than the tracking it replaces, not adopting is correct. `MEASURED`: 157 of 183 census rows are
`CAPABILITY_NOT_USED`. We are short of surface that pays, not of surface.

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

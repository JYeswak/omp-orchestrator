# 09 — Milestones, done-definitions, and how this plan is validated

Serves **R1** ("define what done looks like at each milestone") and **R3** ("something that could pass an 'investor' test — they can
beat up the plan, find any gaps, and pass or fail us"). Sections 01–08 describe a system. This section states the conditions under which
we are allowed to say it works, and the conditions under which this document should be rejected. Written 2026-08-31.

## 1. The done-definition template

**A milestone is closed by an OBSERVABLE, not by a claim** — not by a passing test suite, a commit, an agent reporting success, or a
human's recollection that it seemed to work. An observable is a command someone else can run and a result they can read without asking
us what it means. That is the output of the failure this project was stood down over: **a workspace whose own census measured
mechanisms built, tested, hardened, and called by nothing** (§01: twenty-six crates, 379→407 tests
at last count, and a BUILT≠WIRED census whose row set outgrew the twenty-mechanism figure this
text first cited — see the section-map note at the bottom of this file) — evidence code does what its author thought, not that it runs. Every milestone
below is stated in this shape:

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
mirrors the measured crate chain of §04 (finding-dispatch → omp-orchestrator → ack-stage →
receiver-receipt → tick-monitor is the deepest path in the 18-edge DAG), with M1's seam at the
head and M7's window at the tail.

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

**STARTING POINT — with a recorded disagreement with the brief.** Brief §4 lists the *actionable* layer as **BROKEN** — "`idle_panes`
discards `NewlyIdle`; `free_capacity` derives from the same `is_dispatchable` filter." **I measured the current source and disagree:**

```
crates/tick-monitor/src/lib.rs:462-463   is_free_capacity() = ConfirmedIdle | NewlyIdle
crates/tick-monitor/src/main.rs:244      if live.is_free_capacity() && !excluded.contains(...)
crates/omp-orchestrator/src/lib.rs:461       .filter(|p| p.is_free_capacity)
strings ~/.local/bin/tick-monitor     | grep -c NEWLY_IDLE  -> 1
strings ~/.local/bin/omp-orchestrator | grep -c NEWLY_IDLE  -> 0
grep -rn '"NEWLY_IDLE"' crates/omp-orchestrator/src/main.rs -> 0 hits
grep -rn  "NEWLY_IDLE"  crates/omp-orchestrator/            -> 2 hits, both #[cfg(test)]
```

The producer emits `free_capacity` from its own predicate including `NewlyIdle`; the consumer counts its own field;
`crates/omp-orchestrator/src/lib.rs:451-457` comments the former defect, with the regression test
`observed_idle_state_counts_as_free_capacity_before_confirmation` at `src/main.rs:1056`
(predicate asserts `:1062-1063`). `MEASURED`: **the
filter defect is fixed in source** — the brief's row describes the tree at the 4h19m incident, not this commit, and should be amended in
place. What remains broken is the **seam**. `MEASURED`: the producer names the state; the production parser at `src/main.rs:383`
**never** does, deriving capacity from a JSON string list plus a `state == "IDLE"` fallback, so the NewlyIdle branch is exercised only
through a `#[cfg(test)]` constructor mapping a label production never reads. Three predicates agree by convention across a process
boundary with **no shared type and no end-to-end fixture** (`ls crates/omp-orchestrator/tests/` → `no_noop.rs`); `omp-orchestrator` has
no `path-depends-on` edge to `tick-monitor` (brief §3.4).

**RISK.** A live-seam refactor the repo has flagged as needing an owner: `crates/no-shell-gate/tests/wired_lanes.rs:679` records
`Observation` as "REQUIRES A DECISION not an allowance… the `free_capacity` seam". The failure mode is a partial migration adding a
shared type *beside* the two existing structs, producing three dialects where there were two — and the vocabulary that would collapse
the ack dialects is not merely unadopted but **blocked upstream**: `AckKind` and `DeliveryClass` sit behind `#[cfg(feature =
"messaging-fabric")]`, which needs `test-internals`, which upstream issue #46 removed from defaults, and `ObligationLedger` occurs zero
times. **NO-CLAIM.** M1 makes one defect class a compile error; a shared type with a wrong predicate is wrong in both crates.

**UPSTREAM CORROBORATION, declared only.** OMP declares `GuestIdleReconcilerCtx`
(`dist/types/collab/guest.d.ts:9-30`; probed) with a settle-vs-continuation split — the same two-tier idle design
as `NewlyIdle`/`ConfirmedIdle`, arrived at independently. DECLARED only: no wire probe has carried
it; M1's measured three-predicate seam and the RISK above stand unchanged.

### M2 — Selection will run through the graph kernel instead of queue recency

**OBSERVABLE.** From the repository root and the recorded revision, run
`cargo test -p omp-orchestrator --test selection_graph -- --exact graph_and_recency_choose_different_beads --nocapture`
against the checked-in fixture `crates/omp-orchestrator/tests/fixtures/blocking-chain.json`. The test must print one JSON line with
`{"fixture":"blocking-chain.json","graph_bead":"<id>","recency_bead":"<different-id>","selected_by":"bv","revision":"<git sha>"}` and exit 0; the pre-milestone tree fails because no `bv` invocation or test exists, and the post-milestone tree fails if the IDs are equal or `selected_by` is not `bv`. The corroborating source command is `grep -R -n --include='*.rs' 'Command::new("bv")' crates --exclude-dir=tests`, which must return a production hit. **NOT IN SCOPE.** Dispatch — M2 decides *what* is worked; M3 delivers it.

**STARTING POINT.** `MEASURED` — the rule is enforced and the implementation is absent:

```
harness grep, pattern Command::new\("bv"\), path crates   -> No matches found
grep -rhoE 'Command::new\("[a-z...]+"\)' crates/ --include=*.rs | sort | uniq -c   # non-empty, so not a false zero
  11 git · 6 python3 · 6 br · 4 /bin/kill · 2 tmux · 2 /bin/sh · 1 strings · 1 omp · 1 grep · 1 cargo
```

`bv` is spawned **zero** times, while `crates/kernel-only-operator-hook/src/lib.rs:548` refuses raw queue reads with *"raw `br ready` is
blocked; use the `bv --robot-triage` queue kernel"*, allowlisted at `:24`. **We ship a prohibition whose sanctioned alternative nothing
calls**, and six `Command::new("br")` sites read the queue directly.

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

**OBSERVABLE.** On a clean host, from the repository root at the recorded revision, run this exact shell script:

```sh
set -o errexit -o nounset -o pipefail
git rev-parse HEAD
test ! -e target
tmpdir="$(mktemp -d)"
env -i HOME="$HOME" TMPDIR="$tmpdir" PATH="/usr/bin:/bin:/opt/homebrew/bin" sh -ceu 'cargo install --locked --path crates/installer --root "$TMPDIR/omp-m6"'
env -i HOME="$HOME" TMPDIR="$tmpdir" PATH="$tmpdir/omp-m6/bin:/usr/bin:/bin" omp-orchestrator --repo crates/installer/tests/fixtures/foreign-repo --once --evidence "$tmpdir/m6.json"
```
The checked-in fixture path, checkout SHA, `target/` absence, install exit code, first-tick exit code, and JSON evidence must be captured; `$tmpdir/m6.json` must contain `{"install_exit":0,"cache_absent":true,"fixture":"foreign-repo","first_tick_exit":0,"verdict":"progress|named_refusal","revision":"<git sha>"}`. The pre-milestone tree fails because the install/consumer path is absent; post-milestone success requires every named field and exit code. **NOT IN SCOPE.** Unattended operation (M7); M6 is one successful cold first tick elsewhere.

**STARTING POINT.** `MEASURED`: never attempted. The `installer` crate exists and is **isolated** — on neither side of any of the 18
`path-depends-on` edges (brief §3.4), so nothing consumes it. Host coupling shows in the binary table: `bv` under `/opt/homebrew`,
`cargo` a shim at `~/.rch/shims/cargo`, the mirror on `/Volumes/ZestData/…`.

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

**STARTING POINT.** `MEASURED`: the longest unattended interval observed to date is a **failure** — 4h 19m of fleet idleness while every
watchdog reported healthy. We have an unattended duration; it is the duration of an undetected outage.

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

**This checker does not exist.** `MEASURED`: `ls crates/ | grep plan-check` returns nothing; the workspace has 26 crates and none is
this one. It is the sharpest thing an investor can hold against this document: **a plan that asserts a discipline it does not enforce is
exactly the vacuity defect we found in our own census.** Brief §3.3 measured 183 rows meeting a four-field requirement with one distinct
value across all 183 — met and meaningless. §3 is in that position now: eight rules enforced by the author's attention.

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
| The observe layer works and the NewlyIdle filter defect is **fixed in source** — `crates/omp-orchestrator/src/lib.rs:461`, regression test `observed_idle_state_counts_as_free_capacity_before_confirmation` at
`src/main.rs:1056` (contradicts brief §4; see M1) | **The seam cannot silently diverge again.** `MEASURED`: no shared type crosses it; production never names `NEWLY_IDLE` (`grep -rn '"NEWLY_IDLE"' crates/omp-orchestrator/src/main.rs` → 0). Settled by M1. |
| The one hard rule is enforced mechanically — a Rust gate walks `git ls-files` and fails on `.sh`/`.py`, exemption list empty | **The gates bite under attack.** The former **2-of-8 / 4-of-8** figures are **HISTORICAL, UNPROVEN** snapshots and are not a current verdict. Settled by bringing all 8 to four legs plus ADDRESSABLE, and a planted-known-bad campaign per gate. |
| Failing closed with a remediation hint works — `fh` MCP returns a typed `SERVE_INPUT_STALE` naming the moved mirror HEAD (`5dec4212…` → `ecdea397…`) rather than an empty result | **This plan enforces its own discipline.** `MEASURED`: `ls crates/ \| grep plan-check` → empty. Settled by shipping `plan-check` (§4) with its known-bad fixture. |

**NO-CLAIM.** This section defines the conditions under which a milestone may be called done, and records what is and is not proven as
of 2026-08-31 on one repository and one machine. It does not establish that these are the right milestones, that seven is the right
number, that the ordering is minimal, or that closing all seven produces a system anyone wants. It makes no schedule claim — no date,
duration, or effort estimate appears above, because none is measured. The most important row here is `M5 — UNPROVEN`; no rigor elsewhere
substitutes for it.

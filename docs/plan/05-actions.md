# 05 — Every action: intended purpose, and the negative pattern it must refuse

Serves **R10**: *"what the stated intended purpose of each action is with negative patterns."*

An orchestrator is not a program that does things; it is a program that decides, on evidence,
whether it is entitled to do a thing — and the interesting half is the refusal. Each action is
specified twice: as an intent, and as the wrong behaviour it must be structurally unable to do. **An
action whose negative pattern is hypothetical is weaker than one whose negative pattern has a
scar**, so each is marked. `How we know it refused` names the observable that stops a refusal from
being a line in a log nobody reads — measured at **162 consecutive refused ticks whose typed refusal
nobody consumed** (brief §4). Citations are relative to `crates/` and **name the construct, not just
the line**, since a bare line number is unverifiable and drifts; §03 owns the full schema, so
`Inputs`/`Outputs` carry only the contract and the refusal shape; and a not-found is reported only
with the command **and** why its search space was right.

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
asserted a **changed** hash at 20s proves working. Logically right; the code does not do it.
`liveness()` returns `Unproven { why: "gap_too_short" }` at `:541-545`, *before* the `(Working,
Working)` arm at `:551-557` that compares `prev.hash != now.hash` — positive proof of life is
discarded. **Correctly reasoned, incorrectly implemented; recorded as an open defect.** PROJECTED:
hoisting that check above the floor would make `Live` provable at watcher cadence.

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

**How we know it refused.** The match at `:550-587` is **exhaustive with no wildcard arm**: the `_
=> Live` catch-all it replaces hid a freed worker, and `state-wildcard-lint` keeps that from
regressing. **NO-CLAIM:** a two-capture claim about motion — it cannot tell work from a loop.

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
found by a human looking. A second refusal closes the bypass — `DispatchIntent` splits `Bead` from
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
delivery.* **MEASURED — `cp-z42vu`**: *"`ntm --robot-send` returned `successful:["4"]` while the
packet never reached the pane"* (`dispatch-silence-watch/src/lib.rs:10-11`). The wrong behaviour is
*the natural one*: the transport told the truth about its own send and nothing about the receiver —
the most important negative here. Hence the receiver crate's first rule: *"A sender return value is
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
excludes `Observation` from the allowance list so the collision demands convergence, and 13 tests
pass — **and** the binary's 544,697-byte doctor output contains **zero** occurrences of
`Observation`, `CONVERGE`, or `Verdict`, **and** `--help` returns `CONFIG_ERROR unknown argument
--help`. The honest grade is neither PASS nor FAIL: **built, correct, and undiscoverable** (brief
§3.6). **A grading action that can only pass or fail cannot represent it** — `Grade` needs an arm
for *correct and unreachable*. *(b) Refuse a zero from a tool that cannot distinguish "no matches"
from "did not run".* **MEASURED**: shell `grep -rl 'forbid(unsafe_code)' --include='*.rs' crates`
returns **0** where the harness tool returns **55 files**; without `--include` it is correct. A
second mechanism hit a sibling: `--include='*.rs'` pointed at `ntm`, a **Go** repo — structural
absence read as semantic absence. A grade built on either reads as a clean refutation of a true
claim. A sixth refutation this session landed in shipped source rather than in the plan (A11(a)): a
doc-comment defect claim, cited onward as measured because the source presented it that way.

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
yields `NewlyIdle` (`tick-monitor/src/lib.rs:562`), and the next tick's `(Idle, Idle)` yields
`ConfirmedIdle` (`:560`). The substrate separates the same two states: `AgentEndEvent.willContinue`
(`dist/types/extensibility/shared-events.d.ts:154`) marks a continuation that must not read as
settled, and `GuestIdleReconcilerCtx` (`dist/types/collab/guest.d.ts`) reconciles over that
boundary instead of deriving both from one predicate.

**Negative pattern — what this action must REFUSE to do.** *Refuse to leave a finished pane
unreaped, because an unreaped pane is capacity that silently disappears.* **MEASURED**: the
`NewlyIdle` arm exists only because *"The operator spotted a freed worker my classifier had hidden"*
— that transition previously fell through a `_ => Live` catch-all (`:409-419`). The `actionable`
filter defect itself is **FIXED** (commit `-oco`; `is_free_capacity` is now its own field at
`omp-orchestrator/src/lib.rs:162-175`, with the regression test
`observed_idle_state_counts_as_free_capacity_before_confirmation` at `main.rs:1056`) — what
remains broken is the SEAM: the producer's field and the consumer's parser agree by convention
across a process boundary with no shared type, so a future filter change is invisible again
(09 M1). The scar quote stands as history: *"the exact shape that let the fleet sit idle for hours
while the watchdogs reported healthy"* (`omp-orchestrator/src/lib.rs:451-462`, whose comment now
records the fix). **What would Jeffrey do:** `rg -li --type rust -e
'reap(ed|ing)?_pane|pane_reap|kill-pane' --glob '!target' .` in the mirror — the extension filter is
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

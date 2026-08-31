# 05 — Every action: intended purpose, and the negative pattern it must refuse

Serves **R10**: *"what the stated intended purpose of each action is with negative patterns."*

An orchestrator is not a program that does things; it is a program that decides, on evidence, whether
it is entitled to do a thing — and the interesting half is the refusal. Each action is specified twice:
as an intent, and as the wrong behaviour it must be structurally unable to perform. **An action whose
negative pattern is hypothetical is weaker than one whose negative pattern has a scar**, so each is
marked. `Must be true before` is the caller's predicate; `How we know it refused` is the observable
that stops a refusal from being a line in a log nobody reads — measured here at **162 consecutive
refused ticks whose typed refusal nobody consumed** (brief §4). Future tense is what we will build;
present and past are measured, with the file and line grounding them.

### A1. OBSERVE a pane

**Purpose.** Turn one pane's rendered terminal into a typed, timestamped `Observation`.

**Inputs.** `pane_id: &str`; raw `tmux capture-pane` text; `at: u64` unix seconds. Bytes off a
terminal, therefore untrusted input.

**Outputs.** `Observation { pane_id, state: PaneState, hash: u64, at }`
(`crates/tick-monitor/src/lib.rs:488-494`); `PaneState` = `Working { timer_secs } | Idle | Wedged |
Dialog { timer_secs } | Unproven` (`:220-234`). The refusal shape is `Unproven`: *"not a soft idle…
excluded from idle capacity entirely, because 'I could not read this pane' and 'this pane is free' are
opposite conditions"* (`:218-219`).

**Must be true before.** The capture is for **this** `pane_id` and the read did not time out. A
timed-out `tmux` yields `Outcome::TimedOut` (`:86-89`), whose `stdout_if_completed()` returns `None`
(`:98-103`) — a killed child's empty buffer cannot enter an `Observation` at all.

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse to score the whole buffer.*
**MEASURED**: a whole-buffer scan matched a stale spinner in scrollback and *"one pane scored working
AND idle simultaneously while genuinely idle"* (`:288-292`); the fix is `last_status_line()`, anchored
on the last non-blank non-decoration line (`:293-308`), which also blocks the inverse false positive —
a braille glyph inside quoted prose is not pane state. *(b) Refuse to treat a sub-floor sample gap as
evidence.* **MEASURED**: a 30-second window called two live panes frozen, because a lane deep in a long
tool call has a static timer and changing output; the floor became `MIN_GAP_SECS = 75` and the
asymmetry is in the constant — *"a missed freeze costs idle minutes, a false freeze destroys work in
flight"* (`:479-485`). **Disagreement with the brief:** my spawn instructions asserted a **changed**
hash at 20s proves working. Logically right; the code does not do it. `liveness()` returns
`Unproven { why: "gap_too_short" }` at `:541-545`, *before* the `(Working, Working)` arm at `:551-557`
that compares `prev.hash != now.hash` (`sed -n '540,557p' crates/tick-monitor/src/lib.rs`). Positive
proof of life is discarded. **Correctly reasoned, incorrectly implemented — recorded here as an open
defect rather than described as though it already held.** PROJECTED: hoisting a changed-hash check
above the floor makes `Live` provable at watcher cadence while `Frozen` and `ConfirmedIdle` stay behind
the 75-second floor.

**How we know it refused.** `PaneState::Unproven`, excluded from every capacity list rather than
defaulted into one; sub-floor refusals carry a machine-readable `why: &'static str` —
`"gap_too_short"`, `"pane_id_mismatch"`, `"prior_capture_unusable"`, `"prior_awaiting_answer"`,
`"capture_unrecognised"` (`:506-586`) — so a harness asserts on the cause, not a boolean.

**NO-CLAIM:** proves what one status line looked like at one instant. Not that the pane is healthy,
its agent progressing, or the capture not mid-render.

### A2. CLASSIFY liveness

**Purpose.** Compare this tick against the previous one and emit the single `Liveness` verdict a
dispatcher, a conductor, and an alarm each act on differently.

**Inputs.** `prev: Option<&Observation>`, `now: &Observation` (`crates/tick-monitor/src/lib.rs:496`).

**Outputs.** `Liveness` = `Live | ConfirmedIdle | NewlyIdle | Frozen | Dialog { timer_secs } |
Obscured | Wedged | Unproven { why }` (`:403-440`), plus four predicates so consumers never re-derive
policy: `is_dispatchable()` (`ConfirmedIdle` only), `is_free_capacity()`
(`ConfirmedIdle | NewlyIdle`), `needs_answer()` (`Dialog`), `needs_attention()`
(`Dialog | Obscured`) (`:456-476`).

**Must be true before.** `prev.pane_id == now.pane_id`, checked before every use of `prev.state`
(`:510-516`) — a prior observation from a different pane is not evidence about this one.

**Negative pattern — what this action must REFUSE to do.** *Refuse to key liveness on a marker regex.*
The authority is `stable_hash()`, which strips every braille frame, the `π` glyph, and every parsed
timer before hashing (`:380-396`); the marker is a human convenience. **MEASURED**: a tool-call box
border rendered *after* the status line produced `<no marker>` on two live panes, which a marker-keyed
classifier reads as absence of life. Recorded again from a second watcher: *"%1414 at 09:38Z: a
box-drawing region briefly covered the status line of a pane that was mid-work at 26/26, and the
watcher reported DIALOG"* (`:525-529`). The response was not a better regex but a new arm, `Obscured`,
discriminated by the **prior** observation rather than this capture's shape (`:517-539`). **MEASURED**,
and why `Dialog` exists: `%1372` sat 26+ minutes on an install approval reading as `WORKING`/`LIVE`,
*"so the escalation it was waiting on was invisible to the conductor while looking perfectly healthy"*
(`:422-428`). A timer advancing while a pane blocks on a human is motion, and motion is not work.

**How we know it refused.** The match at `:550-587` is **exhaustive with no wildcard arm**: the
`_ => Live` catch-all it replaces hid a freed worker, filed as
`omp-orchestrator-transition-to-idle-misread-oco`. A new `PaneState` arm is a compile error here, not
a silent absorption into `Live`; `state-wildcard-lint` keeps that from regressing (brief §3.5: 9
tests, 1 known-bad, 1 known-good, 1 mutation).

**NO-CLAIM:** a two-capture claim about motion. It does not distinguish useful work from a loop, and
`Live` is not a statement about correctness.

### A3. SELECT work

**Purpose.** Choose which ready beads to offer next, by graph position, so the fleet works the
critical path rather than the operator's most recent discovery.

**Inputs.** `br` ready rows as JSON; the `bv` ranking; an epic scope string; `QUEUE_WANT` clamped to
`1..=20` (`crates/loop-queue-filter/src/lib.rs:134-135`); a cooldown file and window.

**Outputs.** A bounded, ordered selection of leaf bead ids. The refusal shape is an empty selection
with a named reason — never an empty selection that reads as "nothing to do".

**Must be true before.** The queue was **readable**; `decide()` returns
`SupervisorDecision::QueueUnreadable { detail }` when it was not
(`crates/omp-orchestrator/src/lib.rs:441-445`) — a fail-closed arm, not a zero count.

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse to cherry-pick by the
operator's recency of discovery.* **MEASURED**: the top-3 PageRank items were unclaimed while the
conductor hand-picked recent finds. Recency of discovery is a fact about the observer; graph position
is a fact about the work — selecting on the former turns a ranked queue into a random one while
producing a transcript that looks deliberate. *(b) Refuse to read `actionable_count` as available
work.* **MEASURED**: it includes in-progress beads, so a fleet with everything claimed and nothing free
reports a healthy positive number — the denominator defect brief §3.2 retires and §6 rule 4 forbids.
Available work is `ready ∧ unclaimed`, counted and reported separately. *(c) Refuse to dispatch an
accounting node.* An epic is a parent, not a task: `issue_type != "epic"` (`:399`) and
`id.starts_with(&config.epic) && id != config.epic` (`:416`), pinned by
`epic_exclusion_rule_never_selects_parent` — *"parent accounting node must never be dispatched"*
(`:637-648`) — **MEASURED** as a live test leg, same case in the differential oracle
(`crates/loop-queue-filter/tests/differential.rs:141-145`).

**How we know it refused.** The selection is empty **and** the reason is typed. There is deliberately
no arm meaning "nothing to do": an empty queue beside free capacity is
`QueueEmptyNeedsJosh { free_capacity_count }`, *"a decision only Josh can make, so queue-empty must
remain SUPERVISED rather than silently tolerated"* (`crates/omp-orchestrator/src/lib.rs:379-385`).

**NO-CLAIM:** selection ranks work. Not that a selected bead is well-specified, that its acceptance
criterion is checkable, or that a worker can finish it.

### A4. ADMIT a dispatch

**Purpose.** Decide whether **this** pane may receive **this** packet right now — the last gate before
an irreversible send.

**Inputs.** Pane id, session, owner, an absolute state directory, an absolute ready-probe path
(`crates/pane-dispatch-fence/src/main.rs:71-84`; non-absolute paths rejected, session/pane/owner
constrained to `[A-Za-z0-9._-]+` at `:36-41`).

**Outputs.** Admission, or one of three named refusals as exit codes: `EXIT_BUSY = 75`,
`EXIT_NOT_FREE = 76`, `EXIT_CONFIG = 78` (`:16-18`). A refusal is a distinct code, not a generic 1.

**Must be true before.** `Liveness::is_dispatchable()` — `ConfirmedIdle` and nothing else
(`crates/tick-monitor/src/lib.rs:456-458`). One idle capture is one capture; `NewlyIdle` is visible as
capacity and deliberately not fillable (`:409-419`).

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse a pane advertised free that is
wedged.* **MEASURED**: the composite reads a pane as available on
`observation_state == "idle" && safe_to_dispatch == true`
(`crates/fleet-composite/src/main.rs:315-316`) — and a wedged pane satisfies both while running
nothing. The signature is literal: `classify()` returns `PaneState::Wedged` on *"Press up to edit
queued messages"* or *"Messages to be submitted after next tool call"*
(`crates/tick-monitor/src/lib.rs:352-355`); a packet sent there parks in the composer and never
submits. `Wedged` is checked **first** in `liveness()` (`:497-499`) and is in neither
`is_dispatchable()` nor `is_free_capacity()`; the receipt layer names it independently as
`ReceiptReason::WedgedUnsubmitted` → `WEDGED_UNSUBMITTED`
(`crates/receiver-receipt/src/lib.rs:45-46`, `:84`). *(b) Refuse to admit on a standing verdict.* An
authorization that does not expire is a permanent bypass with a friendly name;
`AuthorizedIdle { pane_count, expires_at }` carries its deadline *"so the decision names its own
deadline"* (`crates/omp-orchestrator/src/lib.rs:376-378`). **PROJECTED — no measured incident yet** of
an expired token honoured; the expiry is in the variant, not a config file, so it is unavoidable at the
match site.

**How we know it refused.** A distinct exit code plus one stderr line naming pane and condition.
Admission is serialised by an OS file lock (`TryLockError` at `:9`, `:31-34`), so two concurrent
admissions cannot both win and the loser reports `EXIT_BUSY = 75` rather than proceeding.

**NO-CLAIM:** proves the pane could accept a packet at the instant of the check. Not that the packet
arrived — that is A6 and A7, and conflating them is the defect A6 prevents.

### A5. CLAIM a bead

**Purpose.** Bind one bead to one assignee **before** the packet is sent, so the dispatch is visible to
the follow-up detector and to every other agent.

**Inputs.** `BeadSnapshot { id, title, description, status: BeadStatus, assignee }` from a
point-in-time `br show --json` projection (`crates/dispatch-claim-fence/src/lib.rs:47-72`), plus a
`DispatchIntent`.

**Outputs.** A `DispatchPermit`, or a typed refusal. The header states the boundary: *"A
`DispatchPermit` does not attest that transport occurred; the dispatch ledger remains the authority for
that separate claim"* (`:3-7`).

**Must be true before.** The status admits dispatch. `BeadStatus` parses into a closed set
`Open | InProgress | Closed | Blocked | Deferred | Unknown(String)` (`:12-32`) — an unrecognised
tracker string becomes `Unknown` carrying the literal rather than being coerced to a known arm.

**Negative pattern — what this action must REFUSE to do.** *Refuse to send a packet naming an unclaimed
bead.* The order is **file → CLAIM → dispatch**, and the middle beat is not optional because the
follow-up detector keys on `assigned ∧ in_progress ∧ no-comment`. An unclaimed dispatch is therefore
**invisible to the detector built to notice a silent worker**:
`dispatch-silence-watch::classify()` takes `current_assignee` and `dispatch_assignee` as required
parameters (`crates/dispatch-silence-watch/src/lib.rs:108-115`) and has a `Reassigned` arm — *"the
original dispatch is moot regardless of whether comments exist"* (`:32-34`). With no claim there is
nothing to compare, so the bead cannot be silent, only absent. **MEASURED by consequence rather than
one timestamped incident**: brief §4 records that every completion this session was found by a human
looking. A second refusal closes the obvious bypass — `DispatchIntent` splits
`Bead { bead_id, receiver_agent }` from `Broadcast { operation, .. }` and
`Correction { operation, .. }` precisely *"so they cannot bypass the bead fence by supplying an empty
bead identifier"* (`crates/dispatch-claim-fence/src/lib.rs:100-117`).

**How we know it refused.** No permit is issued, so A6 has nothing to consume — enforced by the absence
of a value rather than a checked boolean. The permit is A6's required input.

**NO-CLAIM:** records intent to work. It does not reserve files, stop a second agent editing the same
paths, or survive a tracker write that fails silently.

### A6. DISPATCH

**Purpose.** Transmit one packet to one admitted pane and retain the transport's own evidence verbatim,
at the instant of the send, before any later observation can overwrite it.

**Inputs.** A `DispatchPermit` (A5), an admitted pane (A4), the packet text, and a `TransportKind` —
`NtmRobotSend` or `TmuxSendKeysLiteral` (`crates/ack-stage/src/lib.rs:20-26`).

**Outputs.** A `TransportReceipt` retaining `raw_json` plus parsed `targets`, `successful`, `failed`,
`blocked` (`:42-50`, `:113-135`), or a `TransportReceiptError` =
`InvalidUtf8 | InvalidJson | NotAnObject | MissingField | WrongFieldType` (`:69-77`). Unparseable
transport output produces a **named error**, never an assumed success.

**Must be true before.** The transport supports a delivery claim at all.
`TransportKind::supports_delivery_claim()` is true for `NtmRobotSend` only (`:36-39`), *"the only
transport with a retained per-target JSON receipt"* (`:22-23`); the tmux fallback is retained as a
`TmuxSendKeysMeasurement` and surfaces as `ReceiptReason::UnprovenTransport { transport }`
(`crates/receiver-receipt/src/lib.rs:54-57`).

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse to treat `success:[N]` as
delivery.* **MEASURED — `cp-z42vu`**: *"`ntm --robot-send` returned `successful:["4"]` while the packet
never reached the pane"* (`crates/dispatch-silence-watch/src/lib.rs:10-11`). This is the most important
negative in this document, because the wrong behaviour is *the natural one* — the transport told the
truth about its own send and nothing about the receiver. Hence the receiver crate's first rule: *"This
crate deliberately never sends input… A sender return value is therefore never part of the receipt
proof"* (`crates/receiver-receipt/src/lib.rs:5-7`). *(b) Refuse to bypass a guard without recording
what the bypass also skipped.* A bypass that logs "overridden" discards the guard's **true** positives
along with its false one. The answering shape exists in a sibling gate:
`pre-delete-citation-check`'s override *"names the superseding artifact and the caller writes a comment
onto each affected bead"* (`crates/pre-delete-citation-check/src/main.rs:5-7`) — a durable trace on
every row it stepped over. **PROJECTED — no measured incident yet** of a dispatch bypass losing a true
positive; written here because it was raised in conversation, and R11 makes an unwritten requirement a
dropped one.

**How we know it refused.** No `TransportReceipt` is constructed; the failure is a typed
`TransportReceiptError` whose `Display` names the missing field by name (`:79-91`). A caller cannot
reach A7 holding a receipt-shaped hole.

**NO-CLAIM:** proves what the transport reported. Per `cp-z42vu` it proves **nothing** about arrival.
Arrival is A7 and only A7.

### A7. VERIFY a receipt

**Purpose.** Decide, from receiver-side evidence only, whether the packet actually landed.

**Inputs.** `pane_id: &str`, `pre_send: &Observation`, and `PostSendObservation` =
`Present(Observation) | Absent | EmptyPaneList | Missing` (`crates/receiver-receipt/src/lib.rs:24-34`).

**Outputs.** `ReceiptVerdict` = `ReceiptConfirmed { pane_id, timer_before_secs, timer_after_secs,
stable_content_changed } | NoReceipt { reason } | Dead { pane_id } | Indeterminate { reason }`
(`:118-139`), `reason` one of 15 named `ReceiptReason` arms (`:37-71`); the binary maps these to exit
codes 0 / 1 / 1 / 2 (`crates/receiver-receipt/src/bin/receiver-receipt.rs:61-64`).

**Must be true before.** `pre_send.pane_id == pane_id`, checked first, else
`Indeterminate { PaneIdMismatch { expected, observed } }` (`:194-202`). Confirmation is keyed on the
**pre-send** state: `IDLE → WORKING` confirms only when the new timer is below
`MAX_IDLE_TO_WORKING_TIMER_SECS = 30` **and** the stable hash changed; `WORKING → WORKING` only when
the timer resets **and** stable content changed (`:160`, `:175-186`). Two signals, both required.

**Negative pattern — what this action must REFUSE to do.** *Refuse to read a timeout as a verdict.*
Enforced in the type, not a convention: `Outcome::TimedOut { after_ms, group_killed }` is deliberately
**not** `Completed { code: non-zero }` — *"an empty buffer from a killed child must never map to the
token a genuinely failing subject produces. A caller matching on `Completed` structurally cannot read a
timeout as an answer"* (`crates/tick-monitor/src/lib.rs:75-93`), with `stdout_if_completed()` returning
`None` for `TimedOut` and `SpawnFailed` (`:98-103`). The same refusal appears twice more, both
**MEASURED as live test legs**. An empty `tmux list-panes` census yields `Indeterminate`, never `Dead`:
*"A non-empty `tmux list-panes` census must be represented by `Absent` before this function may report
`DEAD`; `EmptyPaneList` deliberately yields `INDETERMINATE`"*
(`crates/receiver-receipt/src/lib.rs:185-187`), arm `EmptyPaneListNoDeathClaim` rendering as
`NOBODY_DEAD empty_pane_list` (`:47`, `:85`). And in the ack layer a **successful** read-back with no
marker is `Missing`, never `Confirmed` — test `singular_verb_trap_produces_missing_ack`, comment *"the
singular verb reports exit 0 but does not post"* (`crates/ack-spine/tests/ack_detector.rs:25-28`).

**How we know it refused.** A non-`ReceiptConfirmed` arm carrying a named `ReceiptReason`, reachable
via `ReceiptVerdict::reason()` which returns `None` exactly for `ReceiptConfirmed` and `Dead`
(`:151-156`), plus a nonzero exit code distinguishing "not delivered" (1) from "cannot tell" (2). That
distinction is the whole product.

**NO-CLAIM:** proves the pane's timer reset and its stable content changed. Not that the agent read the
packet, understood it, or will act on it.

### A8. GRADE a claim

**Purpose.** Establish whether a reported completion is true, by re-running the cited command and
comparing its output against the claim.

**Inputs.** The bead id, the worker's claim text, and the **cited command** — a claim with no
re-runnable command is ungradeable by construction.

**Outputs.** A grade, its cited transcript, and a verdict. **This is the largest missing type in the
workspace**: brief §3.7 measures **6 Verdict-shaped types with no shared trait**, and `Grade` does not
exist. That is why grading is prose written by a human, and naming it here is the fix's precondition.

**Must be true before.** The claim names a command runnable **on this machine, now**, without the
worker's session.

**Negative pattern — what this action must REFUSE to do.** *Refuse to read the worker's report instead
of re-running the command.* **MEASURED — bead `ipg.17`**, the most instructive case here because
re-running **refined** the claim rather than refuting it. The implementation is real:
`crates/omp-inventory-map/src/types_inventory.rs:176-178` deliberately excludes `Observation` from the
allowance list so the collision demands convergence, and 13 tests pass. **And** the running binary's
544,697-byte doctor output contains **zero** occurrences of `Observation`, `CONVERGE`, or `Verdict`.
**And** `omp-inventory-map --help` returns
`{"status":"ERROR","error":"CONFIG_ERROR unknown argument --help"}`. The honest grade is neither PASS
nor FAIL: **built, correct, and undiscoverable** (brief §3.6). A prose grade would have recorded "done"
and been right about the code; a re-run grade recorded a sixth gate property. **A grading action that
can only pass or fail cannot represent this outcome** — a design constraint on `Grade`: it needs an arm
for *the mechanism is correct and unreachable*.

**How we know it refused.** A stored transcript plus the named discrepancy — claimed token, command
re-run, observed count. A grade with no transcript is a status, which brief §6 rule 1 forbids.

**NO-CLAIM:** proves what that command output at grading time. Not that the command was the right one
to cite; a worker choosing a weak citation is a selection problem this action cannot detect.

### A9. CLOSE a bead

**Purpose.** Retire a bead with evidence, so the close is a durable artifact rather than a status
transition.

**Inputs.** The bead id, the grade from A8, the cited transcript, and structural context — parent epic,
children, blockers.

**Outputs.** A closed bead whose `close_reason` cites at least one re-runnable command or path, or a
refusal to close.

**Must be true before.** A8 produced a grade backed by a transcript, and no child or blocker of this
bead is open.

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse a prose close reason.*
**MEASURED**: the `finding` crate header records a wave with a `WAVE.md` and **29 beads** where *"at
least 8 gaps were named in prose and never filed"*, one being *"a close_reason citing a path that never
existed"* (`crates/finding/src/lib.rs:6-10`). The counting problem is in the same comment: *"Eight is
only what one agent could recall; the true count is unbounded because nothing counts it."* The
second-order failure is that **the refusal scrolls past** — a policy that rejects a prose close by
printing to a terminal has not refused anything, the same shape as the 162 unconsumed refused ticks in
brief §4. *(b) Refuse to close a leaf in a way that closes its parent epic.* An epic is an accounting
node; the selector already refuses to dispatch one (`crates/loop-queue-filter/src/lib.rs:399`, `:416`)
and the closer must refuse the mirror-image error of retiring a parent because its most recent child
landed. **PROJECTED — no measured incident yet** on the close side; the dispatch side is pinned by
`epic_exclusion_rule_never_selects_parent` (`:637-648`), the close side has no equivalent leg today.
*(c) Refuse to close without cited re-run evidence.* **MEASURED**: `cp-3k9jq`'s `close_reason` is 104
characters with **zero** path citations while its comments cite `bin/fleet-composite.py` in three
places (`crates/pre-delete-citation-check/src/lib.rs:14-17`) — *"A gate scanning only close reasons
passes this deletion and the incident recurs."* The gate scans `close_reason` **and** every comment,
reporting which surface matched via `CitationConflict.field` (`:28-37`), both legs pinned by named
tests: `close_reason_citation_is_detected` (`:148`), `comment_citation_is_detected` (`:162`), and a
both-surfaces case (`:192-203`).

**How we know it refused.** The bead remains open, and the refusal is written **onto the bead** as a
comment rather than to a terminal — the only observable that survives the pane. The stand-down reap
found **seven real conditions living only in pane scrollback** (brief §1); scrollback dies with the
pane.

**NO-CLAIM:** proves a command was run and its output recorded. Not that the work is correct, complete
against its acceptance criterion, or free of regressions elsewhere.

### A10. REAP a finished pane

**Purpose.** Recognise that a pane's work has ended and return its slot to the capacity pool, so a
finished worker becomes visible capacity rather than a quiet hole.

**Inputs.** The pane's `Liveness` history and the state of the bead it was dispatched.

**Outputs.** A capacity delta and a reap record naming the pane, the bead, and the terminal state.

**Must be true before.** The transition to idle is **observed**, not assumed: `(Working, Idle)` yields
`NewlyIdle` (`crates/tick-monitor/src/lib.rs:562`), and the next tick's `(Idle, Idle)` yields
`ConfirmedIdle` (`:560`).

**Negative pattern — what this action must REFUSE to do.** *Refuse to leave a finished pane unreaped,
because an unreaped pane is capacity that silently disappears.* **MEASURED**: the `NewlyIdle` arm
exists only because *"The operator spotted a freed worker my classifier had hidden"* — the
`(Working, Idle)` transition previously fell through a `_ => Live` catch-all, and the arm's doc gives
the fix: *"Naming it separately is what makes a just-freed worker VISIBLE without buying a slot by
weakening the two-capture rule"* (`:409-419`). The brief's `actionable` layer is still **BROKEN** for
this reason: `free_capacity` derives from the same `is_dispatchable` filter, which requires *Confirmed*
idle, so a pane at `t=0` is excluded from **both** lists (§4). `decide()` now reads its own field and
names the cost: *"That is the exact shape that let the fleet sit idle for hours while the watchdogs
reported healthy"* (`crates/omp-orchestrator/src/lib.rs:451-462`). **What would Jeffrey do:**
`cd /Volumes/ZestData/dicklesworthstone-mirror && rg -li --type rust -e 'reap(ed|ing)?_pane|pane_reap|kill-pane' --glob '!target' .`
→ 7 files, the load-bearing one `frankenterm/crates/frankenterm-core/src/orphan_reaper.rs` (530 lines).
His answer is stronger than ours and it is a **refusal**: *"A command-line match is not proof that
FrankenTerm created the process, and a PID can be recycled between discovery and signalling. The
mechanism is therefore intentionally inert until subprocesses are registered by owned child handle plus
immutable process identity"* (`:1-14`) — he shipped the reaper **inert** rather than unsound, keeping
the historical classifier `#[cfg(test)]` only, *"retained only in tests… even a positive match must
never result in a process signal"* (`:24-27`). We adopt the identity rule: **reap only what you own,
keyed on immutable identity, never on a name match** — so a reap record carries the pane id *and* the
dispatch that owned it, not a pattern that matched a render.

**How we know it refused.** A pane in `NewlyIdle` or `ConfirmedIdle` with no reap record is itself the
alarm, and it is countable: reaped panes and free-capacity panes are two numbers that must reconcile
each tick.

**NO-CLAIM:** returns a slot. Not that the work the pane was doing finished, only that the pane stopped
— A7 and A8 own that distinction, and `Frozen` is not a reap.

### A11. REFUSE

**Purpose.** The meta-action. Every gate here has exactly one real output, and it is a refusal a
machine can consume; the pass is the uninteresting case.

**Inputs.** Whatever the gate scans, plus the scan set itself — which is an input, not an assumption.

**Outputs.** A typed verdict and a distinct exit code. The reference shape is `no-shell-gate`:
`Verdict::Clean | Verdict::Violations(Vec<Violation>)` (`crates/no-shell-gate/src/lib.rs:85-91`) with a
separate `GateError` for conditions that are neither (`:56-64`).

**Must be true before.** The scan set is **non-empty**. `scan()` returns `GateError::EmptyScanSet` at
the single choke point *"so every caller inherits it"* (`:117-124`), and the error's own message states
the doctrine: *"a gate that scanned nothing reports identically to one that passed, so it is an error,
never a pass"* (`:72-77`).

**Negative pattern — what this action must REFUSE to do.** *(a) Refuse to refuse with exit 0.*
**MEASURED — the `br comment` singular trap**: `br comment <id> <text>` prefix-matches to `br comments`,
prints a usage error to stderr, **and exits 0**; *"An agent that checks only the exit code believes the
comment landed"* (`crates/dispatch-silence-watch/src/lib.rs:13-17`). A refusal presenting as a success
is the most dangerous single failure shape in the system, because it is invisible to every caller doing
the correct thing. Our own exit-code discipline is measured and consistent: `kernel-bypass-gate` exits
**3** on an empty scan set with *"empty scan set — the gate cannot verify what it cannot see"*
(`crates/kernel-bypass-gate/src/main.rs:21-22`); `no-shell-gate` exits **1** on violations and **2**
when it *"could not render a verdict"* (`crates/no-shell-gate/src/main.rs:44-49`);
`pane-dispatch-fence` uses 75/76/78 (`:16-18`); `dispatch-silence-watch` exits **3** for
`TRACKER_ERROR` (`crates/dispatch-silence-watch/src/main.rs:59`, `:63`, `:79`). Refusal, error, and
usage are three different codes in every one. *(b) Refuse without naming the exact command that would
satisfy it.* A refusal that does not say what to run next converts a gate into an obstacle. The model
is already here and it is **external**: `fh`'s MCP surface fails closed with a typed
`SERVE_INPUT_STALE` naming the moved mirror HEAD (`5dec4212…` → `ecdea397…`), and brief §3.7's verdict
is that *"failing closed with a remediation hint is the model, not a defect"* — **MEASURED** as `fh`'s
behaviour today. *(c) Refuse to be over-strict.* An attack-only gate refuses correct work, gets routed
around, and dies slower than no gate at all. **MEASURED**: of 8 gates, `path-literal-guard` has 1
known-bad and **0 known-good** legs (brief §3.5), *"the highest-risk gate in the set."* An over-strict
gate is indistinguishable from the outside from a working one, which is why the known-good leg is
mandatory rather than nice to have.

**How we know it refused.** A distinct nonzero exit code, a typed verdict a harness can match on, and a
message naming both the violation and the satisfying command. All three, or the refusal did not happen
— it was merely printed.

**NO-CLAIM:** every claim in this section about a gate is a **floor-raise**, not a guarantee. A gate
proves shape. A crate can satisfy every gate here and still leak a detached task, kill a pid instead of
a process group, or map a timeout to the token a failing subject produces — and **none of those three
is greppable**.

## The six properties every action's gate must have

Five were doctrine before this session. The sixth was born during it, from A8.

1. **Fires on known-bad**, specimen **in-tree**. An out-of-tree patch harness silently no-ops when its
   index hash misses HEAD, and a gate that no-ops looks exactly like one that passed. **MEASURED**: 2
   of 8 gates have no known-bad leg (brief §3.5).
2. **Passes known-good.** Mandatory — without it, a gate that refuses everything is indistinguishable
   from one that works. **MEASURED**: `path-literal-guard` has 0 known-good legs.
3. **Mutation turns the known-bad RED**, specimen restored byte-identically with the sha reported both
   sides — the only leg proving the *detector* works rather than the *fixture*. **MEASURED**: 4 of 8
   gates have no mutation leg (`00-brief.md` §3.5, recomputed).
4. **Anti-vacuity: an empty scan set is an ERROR, never a pass**, enforced at the choke point so
   callers inherit it (`crates/no-shell-gate/src/lib.rs:117-124`). Brief §3.3 is this property failing
   on our **own** inventory: all 183 census rows carry the four mandatory fields with zero missing and
   exactly **one distinct value** of `must_be_true` — syntactically complete, semantically empty.
5. **The claim is a floor-raise, never a guarantee.** A residual "guarantees", "proves", or "makes
   impossible" in a gate header is itself a defect, because a reader who sees it stops looking.
6. **ADDRESSABLE — one documented command runs it, and `--help` names that command.** Added this
   session because a gate satisfying properties 1–5 was unreachable: `omp-inventory-map --help` returns
   `{"status":"ERROR","error":"CONFIG_ERROR unknown argument --help"}` while the gate behind it is
   correct and tested (brief §3.6). **What would Jeffrey do:** searched
   `rg -l --type rust -e 'fn robot_docs|robot-docs|--robot-docs'` in the mirror → prior art in at least
   three projects, closest `coding_agent_session_search/tests/spec_robot_docs_topics.rs`, which pins
   the **topic-set discipline** rather than only the output bytes: *"every declared topic actually
   produces non-empty output with exit 0, and… invalid topics fail cleanly"*, and *"An invalid topic
   returns exit 2 (usage/parsing error). This proves the parser is actively gating on the accepted set
   rather than silently returning an empty body"* (`:14-23`). A second pattern,
   `franken_markdown/tests/completions_drift_test.rs:1-7`, asserts every clap subcommand is covered by
   bash/zsh/fish completions **and** the man page — a drift test for discoverability itself. We adopt
   both: a topic-set test that every documented command is addressable and exits 0, and a drift test
   that the help surface names every gate.

**NO-CLAIM:** these six are testable properties of a gate's *test suite*, not of the gate's
*correctness*. A suite can satisfy all six and still test the wrong invariant. Property 6 is the newest
and is satisfied by **zero** of the eight gates in brief §3.5 — no gate in this workspace has an
addressability test today, and this document is the first place that requirement is written down rather
than said.

# The dispatch chain, every fork, and whether we can prove it works

**Bead:** `omp-orchestrator-gb28` (successor work), `omp-orchestrator-93lo` (ACK emission)
**Measured:** 2026-09-07 by `pane1-omp-claude`, in response to Joshua's refusal to restart the
dispatcher without proof.

---

## The answer to the question asked: NO. I cannot prove it works end to end.

Joshua asked whether turning the dispatcher back on will *ensure* it works end to end. **It will
not, and nothing in this repository currently establishes that it would.** What follows separates
what is *measured* from what is *unproven*, because the distinction is the whole deliverable.

**What IS measured:** the chain is **WIRED**. Every lifecycle crate is consumed by the supervisor's
own source, and the primary loop reaches the stages that matter.

**What is NOT measured:** that any of it *executes correctly in sequence*. The supervisor has not
run since `HD-0016` stopped it on 2026-09-06. **Static wiring is not runtime correctness** — this
repository's own second rule.

**And the decisive evidence against "just turn it on" is not a gap in the code. It is that a human
ran the loop by hand tonight for hours and found a beat the code does not implement at all.**

---

## Stage coverage — CORRECTED 2026-09-07 after a negative control invalidated the first table

**THE FIRST VERSION OF THIS TABLE WAS BUILT ON AN UNCONTROLLED INSTRUMENT AND SIX OF ITS ROWS WERE
MEANINGLESS.** Joshua's standing rule — *run a negative control on your instrument before you believe
its answer* — was aimed at this document within an hour of it landing, and it fired:

```
calls(zzz_cannot_exist_fn, also_absent_fn)   ->  NO VERDICT EMITTED
calls(run_cycle, reap_finished_panes)        ->  NO VERDICT EMITTED   <- IDENTICAL
calls(run_cycle, ack_stage)                  ->  NO VERDICT EMITTED   <- IDENTICAL
```

**A guaranteed-absent symbol and six of my "unknown" stages produced the same output.** So those rows
carried zero information. **The cause was mine: I probed `reap_finished_panes`, `ack_stage`,
`prepare_bead` and `DispatchPermit` — the first two are CRATE names, not functions, and the last two
do not exist anywhere (`grep -c` → 0 each). I invented them.**

**A second instrument in the same table was equally blind.** Its own negative control:

```
receiver_receipt::  (qualified, known-used)   7
zzz_absent_crate::  (guaranteed absent)       0
```

**So a crate used UNQUALIFIED reads identically to an absent one under a `crate::` pattern.**
`dispatch_claim_fence` and `dispatch_silence_watch` appear only as `use` lines — which I nearly
reported as BUILT ≠ WIRED — while the names they import are called bare:
`clears_pending_dispatch_intent` **4×**, `SilenceVerdict` **8×**, plus `authorize` and
`authorize_with_identities`.

### The corrected table — controls run in the same session, both directions

```
NEGATIVE  calls(run_cycle, zzz_cannot_exist)  ->  (empty)      the absent signature
POSITIVE  calls(run_cycle, apply_phase_gate)  ->  confirmed    the instrument discriminates
```

|stage|real symbol|verdict|
|---|---|---|
|observe|`parse_observation`|**confirmed**|
|select|`select_dispatch_order_with_pagerank`|**confirmed**|
|claim|`authorize` (from `dispatch_claim_fence`)|**confirmed**|
|phase gate|`apply_phase_gate`|**confirmed**|
|ACK|`step` (from `ack_spine::ledger`)|**confirmed**|
|silence|`clears_pending_dispatch_intent`|**confirmed**|
|grade|`gate_peer_grading`|**confirmed**|
|reap|`finished_pane_reaper_args`|**confirmed**|
|finding|`file_supervisor_finding`|**confirmed**|
|receipt|`classify_ack_wait`|**UNMEASURED** — absent signature; called qualified as `receiver_receipt::classify_ack_wait` at `:3020`, so the probe and the call disagree on the name|
|dispatch (send)|no resolved symbol yet|**UNMEASURED**|

**Nine of eleven stages confirmed, two unmeasured — not "six unknown".** The chain is **more wired
than the first version of this document claimed**, and the error was entirely in the instrument.

**`UNMEASURED` here means the probe returned the absent signature.** It is not a verdict about the
code and must not be read as one — which is exactly the distinction the negative control exists to
make visible.

**CORRECTION to `AGENTS.md`'s crate table:** it lists `reap-finished-panes` as **CONTROL-PLANE**.
It is **PRESENT here**, and I cited that stale row when I first assessed that the reap stage did not
exist locally.

## Every fork: 235 variants across 8 crates

```
ack-spine                 63
receiver-receipt          41
ack-stage                 39
dispatch-claim-fence      35
loop-queue-filter         17
finding-dispatch          15
dispatch-silence-watch    16
admission-reason           9
                    ---------
TOTAL                    235
```

The **primary** decision, `finding_dispatch::SupervisorDecision`, has **10**: `GateUnwired`,
`Dispatch`, `EscalateIdleIncident`, `MonitorBlind`, `QueueUnreadable`, `AwaitingHuman`, and four
more.

**The forks that decide whether a dispatch survives:**

```
DispatchPermit    Bead | Broadcast | Correction                                        (3)
ReceiptVerdict    ReceiptConfirmed | AckConfirmed | NoReceipt | Dead | Indeterminate    (5)
AckAction         RecordReceipt | Retry | Unstick | AwaitHuman | AbandonDeadPane
                  | RetryExhausted                                                     (6)
SilenceVerdict    VerdictPosted | SilentPastDeadline | Reassigned | TrackerError        (4)
FollowUpAction    Healthy | NeedsFollowUp                                              (2)
Withheld          PhaseGate                                                            (1)
```

**A `Dispatch` fork that ends in `Indeterminate` is the failure mode with no owner.** `ack-stage`
documents that the tmux literal fallback is **always** `Indeterminate` on receiver heuristics alone
— so on that transport the ACK comment is not one evidence source among several, it is **the only
one the design admits.**

---

## THE GAP THAT MATTERS MOST: a beat the code does not implement

The documented lifecycle is

```
file → claim → dispatch → ACK → observe → verify → close
```

**Tonight it was executed by hand, and it stalled three beads at once on a beat that is in no
crate:**

```
file → claim → dispatch → ACK → observe → verify → RELEASE → close
                                                    ^^^^^^^
                          the author must hand the bead back before a
                          non-author can claim it for grading
```

Two panes refused grade dispatches in under a minute, both reporting `status=in_progress` with the
**author still holding `assignee`**. Both refusals were correct. **An implementation-complete bead
assigned to its implementer is not grade-ready however the packet is worded**, and the dispatcher is
the only party positioned to notice, because it is the only one that knows both the author and the
intended grader.

**A restarted dispatcher walks into this immediately.** Its `grade` stage cannot route to a
non-author while the author holds the row, and nothing in the 235 forks refuses that state — it
simply produces a dispatch that the receiving pane declines.

---

## And the eligibility field the grade stage would key on is not unique

```
%7   WildStone
%8   WildStone      ← one name, THREE panes
%9   WildStone
%19  PearlGate
%20  pane20-omp-claude
```

**An agent name is a persona shared across panes, not an identity.** So `AGENTS.md`'s bar —
*"DIFFERENT PANE, not different lineage"* — is load-bearing rather than stylistic: `pane=` is the
only unique field in an assignee string.

**Three fields are useless for authorship**, all measured tonight:

|field|why it fails|
|---|---|
|`created_by`|**321 of 975 (33%)** read `josh`, who wrote none of them. Root cause: `crates/finding`'s own publisher omitted `--actor`, so the **sanctioned** filing path was the misattributing one|
|`assignee` agent half|a shared persona — see above|
|text-mention scan|a pane scanning for its own name found **43** comments; authored-signature narrowing gave **12**. The 31 difference was peers quoting it|

**Fixed since:** the `finding` kernel now refuses without an actor (`FINDING_ACTOR_UNSET`), proven
end to end — `finding file … --actor WildStone` → bead `zm55` reads `created_by=WildStone`, and the
omitted-actor arm exits 3 with nothing spooled.

---

## Ranked gaps — what a restart hits, worst first

|#|gap|status|evidence|
|---|---|---|---|
|1|**RELEASE beat unimplemented**|**prose only** (`AGENTS.md` 8-series)|3 beads stalled simultaneously; 2 panes refused|
|2|**ACK instruction not emitted by the dispatch site**|`93lo` open|every packet written before it omitted the instruction; **zero** ACK comments existed, so a landed dispatch read as `unproven_transport`|
|3|**Close-reason policy is unenforced**|**measured tonight**|`close_reason: "PROBE"` was **accepted** on the S0 epic, where `AGENTS.md` says a prose reason "is refused by policy"|
|4|**Eligibility cannot key on any single field**|3 fields refuted|see table above|
|5|**`refill-idle-panes --plan` proposes pane 1**|the orchestrator itself|no `OMP_*` exclusion var in the binary's strings|
|6|**`safe_to_dispatch` is not liveness**|documented|a wedged pane accepts a packet and parks it at `Press up to edit queued messages`|
|7|**`HD-0016`'s own release condition cleared UNREAD**|**measured tonight**|both branches met (`jplf.1.2` closed; `jplf.9` in `br ready`) while the dispatcher stayed down|
|8|**Six stages have unresolved reachability**|this document|`claim`, `dispatch`, `ACK`, `receipt`, `silence`, `reap`|

**Gap 3 and gap 7 are the same shape as gap 1:** a rule this repo treats as mechanical is actually
prose, and nothing fires when it is violated.

---

## The dogfood answer: tonight WAS the dogfood, and it is the only kind that has found anything

**Every gap above was found by a human plus four panes executing the loop by hand.** None was found
by a test, a gate, or a static scan — and the repo has 235 forks, dozens of green suites, and a
flagship gate that *"the exemption list is empty by design."*

That is the measurement worth acting on: **the manual loop is the highest-yield instrument we have,
and it is unranked and unrecorded.**

### What to run before any restart

1. **Resolve the six `unknown` stages** to `confirmed` or `refuted`, per stage, with the probe
   output pasted. `ripwire --verify='calls(A,B)'` is three-valued; treat `unknown` as unmeasured.
2. **A shadow tick.** Run the supervisor with dispatch **disabled** and record what it *would* have
   decided for one full cycle, then diff against what the humans actually did tonight. **A
   disagreement is a defect; agreement is the first evidence the loop can follow the chain.** This
   is obtainable today: `apply_phase_gate` is pure, and `gate_active=false` is observable — the
   clause that refuted the earlier deferral.
3. **One end-to-end bead, driven by the binary, watched by a human.** Dispatch → ACK → grade →
   release → close, on a throwaway bead. **Every one of the 8 gaps above is a checkpoint.**
4. **Rank by fork coverage.** 235 variants; a restart exercises a handful. Record which forks the
   shadow tick and the live bead actually reached — an unreached fork is untested however green the
   crate is.

### The gate that would have caught gap 1

A dispatch site that **refuses to route a grade while the bead's `assignee` names a different
pane.** That is one predicate, it is mechanically checkable, and it is the difference between the
release beat being doctrine and being enforced.

---

## NO-CLAIM

**This document proves the chain is wired. It does not prove it runs.** Six stages remain
`unknown` — unresolved probes, not refuted calls. No supervisor tick was executed for this
assessment; every figure is a static census over the working tree, and per this repo's rule 8 a
worktree is not a commit in a five-agent shared checkout.

The 235-fork count is a census of `pub enum` variants in eight crates on the dispatch path. It
counts **declared** states, not reachable ones — an unreachable variant inflates it, and nothing
here establishes which are live. **The number is a floor on complexity, not a coverage denominator.**

The eight ranked gaps are the ones tonight's manual run surfaced. **The list is not exhaustive and
its ordering is judgement, not measurement** — a shadow tick would reorder it, which is precisely
why step 2 exists.

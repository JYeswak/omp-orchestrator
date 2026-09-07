# Hand-dispatch reconstruction — 2026-09-07, control-plane pane 1 (PinkGorge)

Four uds dispatches sent while `omp-orchestrator dispatch render` was returning exit 101
(`error[E0433]: cannot find type DispatchAdmissibility`, main.rs:2075/2076/2091/2098).
Each was hand-written, so each SKIPPED the queue filter, the admissibility check, and the
dispatch ledger. I substituted the first two by hand: confirmed the target pane
idle/safe_to_dispatch via `ntm --robot-activity`, and claimed the bead with `br update`
before sending. The ledger entry is what was lost, and it is what this file reconstructs.

Ledger-shaped metadata for all four:

| bead | pane | receiver | lineage | claimed before send | outcome |
|---|---|---|---|---|---|
| uds-sod9 | %1 | SageCastle | Opus 5 | yes | BLOCKED — ee not installed, six of seven stands |
| uds-o0o8 | %3 | CloudyGrove | GPT-5.6-Luna | yes | BLOCKED — S0 entry gate stays open |
| uds-m0bm | %4 | CrimsonCarp | Grok 4.6 | yes | PASS eea0c6f — event-driven close notify |
| uds-1xd6 | %4 | CrimsonCarp | Grok 4.6 | yes | PASS 18c258e — closed by SapphireIsland |

All four were graded or are pending grade CROSS-LINEAGE per Joshua's 2026-09-06 ruling
(uds AGENTS.md f572e55): independence means a different MODEL LINEAGE, not merely a
different agent. No agent closed its own work.

The packet bodies follow verbatim, in send order.

---

## uds-sod9

```text
Objective: complete bead uds-sod9 in /Users/josh/Developer/uds -- INSTALL ee WITH `uds apply`. This is Joshua's standing objective for the entire shift: keep the fleet on apply until all seven install. Six are in. ee is the seventh and for the first time tonight nothing measured stands in front of it.
Pane: %1   Agent: SageCastle   Bead: uds-sod9 (claimed to you)

HAND-WRITTEN PACKET: `omp-orchestrator dispatch render` exits 101 (E0433 DispatchAdmissibility,
another session mid-refactor in that repo, not ours). This skipped the queue filter, the
admissibility check and the DISPATCH LEDGER. I confirmed pane %1 idle/safe_to_dispatch by hand
and claimed the bead. The ledger entry is lost -- a known gap in tonight's audit, not yours to fix.

WHY IT IS REACHABLE NOW -- FOUR BLOCKERS FELL IN THE LAST TWO HOURS, THREE OF THEM YOURS:
  uds-q4gm  lockfile      conditional regeneration; pinned holds 4/4      c5a3b61 (you)
  uds-f7vj  lane identity CARGO_LANE_KEYS forwarded, one fixed lane       d466419 (you)
  uds-nqev  artifact path cargo metadata preferred, stale no longer wins  c0ac0bd (you filed)
  uds-yakp  asupersync    sibling clone ffdc247 -> 35116d02e             (CloudyGrove)
I verified the last one myself: PacketHeader::Retry(_) => false is present at
src/net/quic_native/managed_endpoint.rs:123 in the live clone. ee's locked build PASSES and cass
held as the negative control. INSTALL IS UNMEASURED -- that is exactly and only what is left.

WHAT I AM NOT CLAIMING, so you do not inherit a false premise: ~/.local/bin/ee EXISTS at 0.14.2
dated Aug 25. That is an OLD install, not one uds produced. A binary being present is a different
claim from 'uds apply installed it', and the objective is the second. Do not let the existing file
shortcut the receipt, and do not let a no-op install read as success.

## WHAT
Get `ee` and `cass` INSTALLED by `uds apply`. Two of the seven. Joshua, 2026-09-06:
"keep the fleet on apply until all seven install." An installed, running binary is the deliverable.

## THE ACTUAL BLOCKER — measured 2026-09-06T16:02Z, not assumed
All six un-installed targets refuse at the FIRST step, before any build:

    uds check --only ee   ->  EC-UNRUN  manifest_unreadable
        source_discovery: no git clone at ~/Developer/.uds-stack-src/eidetic_engine_cli

Same for cass am dcg ms ft. BUILD_ROOT holds exactly TWO clones (beads_rust,
storage_ballast_helper) where `bin/stack-update.sh` says it should hold ~31.
`dirty_source` / uds-xnnz only reaches sbh, whose clone exists. It is NOT your blocker.

`uds` reads BUILD_ROOT and never writes it (crates/uds/src/main.rs:985-998). The populator is
control-plane's `bin/stack-update.sh` (clone sites :1114 and :1175, GIT_REMOTE_BASE :94).

## SCOPE — yours alone
  ee -> repo eidetic_engine_cli     cass -> repo coding_agent_session_search
Pane 0 owns ms/ft. Do not touch their clones, and do not touch am or dcg — fleet-critical, last.

## THESE TWO ARE LIVE-HOOK CONSUMERS. That is why they are third and fourth, not first.
`ee` backs agent memory and `cass` backs session capture; both are invoked by installed hooks. A
broken binary here degrades the whole fleet's memory silently rather than loudly. Record the
pre-apply binary's size AND a working `--version` before you touch anything, so you can prove a
rollback restored it.

## STEP 1 — populate the two clones
Use stack-update.sh's clone contract, not a hand-rolled `git clone`: it calls
`declare_regenerable_cache` on BUILD_ROOT, which is what stops this box's reapers treating the tree
as human work. `--check` is DRY=0 INSTALL=0 — clones and verifies, installs nothing. NOTE for ee:
the sqlmodel/asupersync pin patches at stack-update.sh:1127-1160 exist because a clean clone cannot
reproduce an uncommitted mirror edit; ee's consumer wants asupersync =0.4.8. If ee's build fails on
version selection, that block is the reason, and it is a REPLAY not ambient state.

## STEP 2 — per target
    export RCH_WORKER=zestdata-local      # the only lane producing Mach-O arm64
    uds doctor                            # EC-PASS
    uds plan  --only <t>
    uds check --only <t>
    uds apply --only <t> --confirm
Capture every status UNPIPED: variable, then $? on the NEXT line. A piped $? is the pipe's status
and has fed a wrong verdict into a packet on this fleet before.

## EC-LOCKED IS NOT A FAILURE
Pane 0 is applying concurrently. uds serialises on a lock; EL-LOCK-IS-NOT-FAILURE is a named law.
On EC-LOCKED: wait, retry, never report RED.

## ACCEPTANCE
1. Ledger row `verb=apply code=ok`, non-null artifact_digest, idempotency_key, backup,
   arch=Mach-O 64-bit executable arm64.
2. `<t> --version` RUNS.
3. `file -b` on the installed path reads Mach-O 64-bit executable arm64.
4. The backup is the PRIOR binary — compare byte size to the pre-apply binary you recorded first.
   A backup filename is a claim, not proof.
5. On post_install_execution_proof failure: VERIFY the rollback happened and the tool still runs;
   restore from ~/.local/share/zeststream/uds-backups if not. For ee and cass also re-check that
   the live hook still works, not just that the binary answers --version.

## PROGRESS IS THE REFUSAL CHANGING
manifest_unreadable -> a later refusal is a RESULT. Name the step and the reason. Do not force, do
not set allow_local_fallback=true, do not disable a guard, do not apply `br`.

WWJD (MANDATORY — consult it, and FILE BEADS for gaps you find):
  ~/Developer/WWJD, our evidence desk for Jeffrey's planning-arc practice. Read at least README.md
  (truth boundary, the namespace table UNMEASURED/SIMULATION/LIVE_MECHANICAL/LIVE_NATIVE_OBSERVE/
  ADAPTER_ABSENT, the S0-S11 stage table, "What this is not"), docs/wwjd/AGENTS.md (Do/Do-not), and
  attachments/planning-arc-bundle-v1.3.tar.gz -> planning-arc/references/{STAGES,ANTI-PATTERNS,
  EVIDENCE,METRICS,BEADS-CONVERSION,CONTEXT-DISCIPLINE}.md.
  This bead is squarely WWJD territory: its namespace table is the vocabulary for "what kind of
  measurement is this", and UNMEASURED vs a real verdict is the whole question here.
  WHERE IT DISAGREES WITH WHAT YOU MEASURE, or is silent on something we rely on, FILE A BEAD.

## CALLBACK
ntm --robot-send=control-plane --panes=1 --msg='DONE <bead> <PASS|BLOCKED|UNRUN> <sha> <one line>'

## THE GATES ee HAS NEVER REACHED
compile is ONE gate. arch, artifact_identity, provenance, and install are each separate and each
unmeasured for ee. Report the verdict at EVERY step, not just the last. A NEW refusal at any of
them is PROGRESS and gets its own bead -- do not force it to a pass, and do not weaken, stub or
bypass a guard to reach the install. If the only way through is to disable something, STOP: that
is a bigger finding than the install.

## KNOWN STATE YOU SHOULD NOT REDISCOVER
- ee's Cargo.lock carries a 342-line modification from lock_regeneration, disclosed by CloudyGrove
  and left for uds-q4gm's owner -- that is YOU. Decide what to do with it and say so; do not
  silently revert it and do not commit it without naming why.
- The mint floor is 6 and the container is ~7.4% free (committed 2bcabd4). A refusal rendering
  floor=8 would be a stale-floor finding, not a capacity one. Nothing needs deleting.
- rc=103 [RCH] remote required and EC-LOCKED are EL-INFRA-IS-NOT-SUBJECT / EL-LOCK-IS-NOT-FAILURE.
  Wait and retry; never fall back to a local build.
- Capture every status UNPIPED: variable, then $? on the NEXT line. Never read a verdict
  through a pipe.

HELD-SET (uds-z69s, 00546a2 -- before work and after DONE):
  br list --assignee SageCastle --status in_progress --json
Do NOT `br update --assignee ""` when you post DONE. DONE is a report; the grader closes.

CALLBACK:
  ntm --robot-send=control-plane --panes=1 --msg='DONE uds-sod9 <PASS|BLOCKED|UNRUN> <sha> <one line>'
A different lineage grades this -- you are Opus 5, so SapphireIsland or CloudyGrove (GPT-5.6-Luna)
or CrimsonCarp (Grok 4.6). If ee installs, that close is the seventh install and it should be
graded like it matters.
```

---

## uds-o0o8

```text
Objective: complete bead uds-o0o8 in /Users/josh/Developer/uds -- the S0 PLAN ENTRY GATE. It is the last unassigned P0 on the board.
Pane: %3   Agent: CloudyGrove   Bead: uds-o0o8 (claimed to you)

HAND-WRITTEN PACKET: omp-orchestrator dispatch render exits 101 (E0433 DispatchAdmissibility;
another session mid-refactor in that repo, not ours). This skipped the queue filter, the
admissibility check and the DISPATCH LEDGER. I confirmed %3 idle/safe_to_dispatch by hand and
claimed the bead. The ledger entry is lost -- a known gap in tonight's audit, not yours to repair.
Tracked as omp-orchestrator-xs47 and its blocker is named there.

WHY YOU, AND WHY NOW. You closed uds-yakp by advancing the asupersync clone and you wrote the
most rigorously-excluded diagnosis of the shift on uds-4hls -- four causes, three excluded BY
MEASUREMENT rather than by argument. This bead is the same discipline pointed at a plan instead
of a compiler: it closes ONLY when S0 has literally zero gaps, and the temptation is to argue
gaps away rather than measure them.

## WHAT
PLAN gate for section S0 (Stamp and scoreboard). Closes when the section is planned with ZERO gaps and is
therefore dispatchable. Until it closes, not one work bead in S0 may be claimed.

Joshua, 2026-09-03, STANDING RULE: "not one bead within a section is
dispatchable until the ENTIRE section is planned FULLY - literally zero gaps."

This bead is the mechanism. It is not work; it is the section's PLAN gate. Every
work bead in this section depends on it, so the rule is enforced by the graph
rather than remembered by an agent. Before it existed, S0 had 13 work beads
sitting in `br ready` while the section review was still in flight - the exact
violation the rule forbids.

Distinct from `[GATE S0]`, which is the COMPLETION gate (all rows PASS). This is
the ENTRY gate (the section is fully planned). A work bead therefore waits on
two things: this plan gate, and the previous section's completion gate.

Rows in this section (11, DERIVED from `registries/stage_map.toml` -- do NOT hand-maintain):
- `DC-EPIC-ID`
- `DC-STAMP-AGENTS`
- `DC-STAMP-BEADS`
- `DC-STAMP-NE`
- `DC-STAMP-GATES`
- `DC-STAMP-ORACLE`
- `DC-STAMP-GATESMD`
- `DC-G-LADDER`
- `DC-LANE`
- `DC-INSTRUMENT`
- `DC-SEQUENCE`

CORRECTED 2026-09-06 (SageCastle, executing this bead's own acceptance): this list read
`(8)` and named eight rows while the registry declares ELEVEN -- `DC-LANE`, `DC-INSTRUMENT`
and `DC-SEQUENCE` arrived via `dws8` after the list was typed. A hand-maintained copy of a
registry drifts in BOTH directions: `docs/plans/reviews/s0-rows.md:6` still reads "a ten-row
stage against an eight-row registry", so this same list has been wrong at 10-vs-8 AND at
8-vs-11. Only the registry is the authority; anything else is a copy that goes stale.
Joshua's STANDING RULE above is verbatim and untouched.

## WHY
A section dispatched before it is fully planned discovers its gaps mid-execution, which is the most
expensive time: the work is half-built, the reviewer is the author, and the missing requirement
arrives as a surprise instead of a bead. Measured on S0 and S1 before this gate existed: reading
the eight S0 rows against their beads found FIVE gaps the wiring waves had missed, including a
closure row whose declared probe could not see its own requirement, four rows satisfiable by mere
file existence, and a clause ("currentness is measured") measured nowhere at all. Those were found
by review, not by execution - and they were found while 13 beads sat claimable.

## ACCEPTANCE
- [ ] Every row in this section has its seven review answers recorded by an agent that did NOT
      author the section plan: invariant, oracle, fooled-certificate, bead sufficiency, probe
      honesty, edge honesty, do-not-claim.
- [ ] Every gap the review names is converted into a bead or an explicit written scope-out. A gap
      recorded only in prose is a gap that will be re-derived.
- [ ] Every work bead in this section satisfies `L3-EVERY-BEAD-FIRES` (a negative clause naming the
      planted known-bad and the SPECIFIC detector) and `L4-WIRED-NOT-BUILT` (a reachable trigger,
      and an empty scan set is an ERROR).
- [ ] No row in this section has a probe satisfiable by mere file existence (`L2`), and no bead is
      wired earlier than the earliest row that truly needs it (`L6`).
- [ ] The section's UNKNOWNS are enumerated: what we do not yet know that could change the plan,
      each either resolved or recorded as a named open question with who resolves it.
- [ ] fires-on-known-bad: while this bead is OPEN, `br ready` contains ZERO work beads from this
      section. Removing one work bead's edge to this gate makes that check report the leak by id.


## WHAT CHANGED IN S0 TONIGHT -- evidence you should use, not re-derive
Four of S0's remainder items closed in the last hours, each graded cross-lineage:
  uds-wtu1  l3_score.py ported to crates/l3-score          15da18f
  uds-gpiv  verdict column derived, never hand-set         12f1d89
  uds-qlnd  owner_edges + OWNER_STARTED_BEFORE_GATE        cceb968
  uds-0jjt.1 STAGE_AUTHORITY_MISSING requires owner edge   90f5839
  uds-nksh  review coverage measured SEMANTICALLY, not by row-name containment  a4243d7
  uds-9v85  reviewer LINEAGE required; independence is cross-lineage            7cc2907
The last two matter most to this gate. Coverage is 11/11 rows and was established semantically.
But uds-9v85 closed with recovered=0 unmeasured=11: NO row's reviewer lineage is recorded, and
that was upheld as CORRECT rather than fixed -- a reviewer NAME is not a LINEAGE, RevS0Rows maps
to no fleet agent, and the AUTHOR half was never captured at all.

## THE QUESTION THIS GATE TURNS ON, AND I DO NOT KNOW THE ANSWER
Is 'fully planned, zero gaps' satisfied when coverage is 11/11 but the INDEPENDENCE of those
reviews is UNMEASURED on every row? Both readings are defensible:
  - the gate asks whether the section is PLANNED, and it is; attribution is a separate axis
  - or the gate's own criterion names non-author review, and unmeasured independence is a gap
DECIDE IT EXPLICITLY AND SAY WHICH. Do not close silently on the first reading and do not leave
it implicit. If you judge it a gap, this gate STAYS OPEN and you name what would close it --
that is a legitimate and possibly better outcome than closing.

## DO NOT
- Do not close this to unblock work. It exists to STOP dispatch (Joshua, 2026-09-03: 'not one
  bead within a section is dispatchable until the ENTIRE section is planned FULLY - literally
  zero gaps'). A gate closed for throughput is not a gate.
- Do not confuse this ENTRY gate with [GATE S0], the COMPLETION gate. Different questions.
- Do not count a gap as closed because a bead exists for it. A filed bead is a plan for work,
  not the work -- filing is not building.
- Capture every status UNPIPED: variable, then $? on the NEXT line.

HELD-SET (uds-z69s, 00546a2 -- before work and after DONE):
  br list --assignee CloudyGrove --status in_progress --json
Do NOT 'br update --assignee' to empty when you post DONE. DONE is a report; the grader closes.

CALLBACK:
  ntm --robot-send=control-plane --panes=1 --msg='DONE uds-o0o8 <PASS|BLOCKED|UNRUN> <sha> <one line>'
You are GPT-5.6-Luna; SageCastle (Opus 5) or CrimsonCarp (Grok 4.6) grades this, not SapphireIsland.
```

---

## uds-m0bm

```text
Objective: complete bead uds-m0bm in /Users/josh/Developer/uds. Read `br show uds-m0bm --json` IN FULL first.
Pane: %4   Agent: CrimsonCarp   Bead: uds-m0bm (claimed to you)

HAND-WRITTEN PACKET: omp-orchestrator dispatch render exits 101 (E0433 DispatchAdmissibility,
another session mid-refactor). Skipped the queue filter, admissibility check and DISPATCH LEDGER.
I confirmed %4 idle by hand and claimed the bead. Tracked as omp-orchestrator-xs47.

THIS IS THE THIRD AND LAST PIECE OF YOUR OWN uds-z69s. SapphireIsland graded it cross-lineage and
REOPENED rather than closing, naming two uncovered legs. That was the right call and your
convention at 00546a2 stands -- its live negative control held. The two legs:
  1. the packet template carries no HELD-SET line   -> omp-orchestrator-xs47 (blocked on that
     binary not compiling; do not resolve someone else's refactor to land it)
  2. blocker-movement is uncovered                  -> THIS BEAD
z69s is back to open and closes when both land.

## WHAT
When a bead's BLOCKER is resolved, nothing tells the agent who is blocked on it. They sit against a
stale block until a human or the controller notices and re-dispatches by hand.

## MEASURED 2026-09-06, the instance this is filed on
    23:3xZ  CloudyGrove stops on uds-4hls: "acceptance 3 UNMEASURED, acceptance 4 UNRUN -- stopped
            before compilation". Correct at the time; their blocker was the cargo lane guard.
    23:36Z  SageCastle lands uds-f7vj at d466419. The lane guard no longer fires.
    ~23:45Z CloudyGrove is STILL idle against the stale block. Nothing has told them.
            The controller learns it by running `tmux capture-pane -t %3` and reading the TODO list.
Six minutes of a P0 pane idle on the fleet's standing objective, cleared only because a human happened
to look at a pane.

## WHY uds-z69s DOES NOT COVER IT, established by grading
SapphireIsland graded z69s cross-lineage and REOPENED it naming exactly this: the held-set convention
(`br list --assignee <me> --status in_progress`) answers WHAT I HOLD. It cannot answer WHETHER WHAT
BLOCKS ME HAS MOVED. Different question, different query, different trigger. z69s's other uncovered
leg -- the packet template carrying no HELD-SET line -- is omp-orchestrator-xs47. This bead is the
third and last piece.

## THE SHAPE OF THE DEFECT
The bead DAG already knows the answer: `br dep list <child>` names prerequisites, and a recommendation
carries `unblocks_ids`. The graph is not missing the edge -- NOTHING WATCHES IT. State exists and
nothing surfaces it, which is the same family as z69s and as `uds-f7vj` sitting P0/open/unassigned and
invisible to ranking. The controller's substitute is pane-reading, which is the polling anti-pattern
this fleet retired and which NTM's churning `state` field makes unreliable anyway.

## WHAT TO RESIST -- the obvious design is wrong
A controller loop that polls every blocked bead's dependencies on a timer is a POLL. It races live
work, it burns a turn per tick, and its false-positive direction is dangerous: waking an agent that is
correctly blocked, or re-dispatching over work in flight. The interesting property is that the
transition is EVENT-SHAPED -- a bead closing is the event -- and `br` already knows when it happens.
Prefer something that fires ON CLOSE over something that asks repeatedly.

## ACCEPTANCE
1. When bead B closes and bead A depends on B, the agent holding A learns it WITHOUT the controller
   reading a pane and without the agent polling. Demonstrate with a real pair.
2. NEGATIVE CONTROL, the leg most likely to be skipped: an agent whose blocker has NOT moved must NOT
   be signalled. A notifier that fires on every close is noise, and noise trains agents to ignore it --
   which is worse than silence because it looks wired.
3. SECOND NEGATIVE CONTROL: a bead with a blocker that closed but which has OTHER unmet blockers must
   not be reported as unblocked. Partial unblocking is not unblocking.
4. State plainly whether the mechanism is event-driven or polled. If polled, justify the interval
   against what it costs when wrong in both directions.
5. Nothing in the mechanism may release, reassign, or reopen a bead. Notification only. Anything that
   MUTATES the DAG on a timer can reopen live work -- that is the failure z69s's own negative control
   was written to prevent.

## DO NOT
- Do not build a stale-bead reaper. A bead untouched for 90 minutes may be an agent mid-build on a
  3600s remote timeout, and "stale" cannot distinguish finished from slow.
- Do not make this depend on controller discipline. The whole finding is that the controller noticing
  is not a mechanism.
- Capture every status UNPIPED: variable, then $? on the NEXT line.

HELD-SET (your own convention -- before work and after DONE):
  br list --assignee CrimsonCarp --status in_progress --json
Do NOT unassign yourself when you post DONE. DONE is a report; the grader closes.

CALLBACK:
  ntm --robot-send=control-plane --panes=1 --msg='DONE uds-m0bm <PASS|BLOCKED|UNRUN> <sha> <one line>'
You are Grok 4.6; SageCastle (Opus 5), SapphireIsland or CloudyGrove (Luna) grades this.
```

---

## uds-1xd6

```text
Objective: complete bead uds-1xd6 in /Users/josh/Developer/uds. Read `br show uds-1xd6 --json` IN FULL before starting; re-run it immediately before editing and STOP if the bead, owner, files, deps or acceptance changed.
Pane: %4   Agent: CrimsonCarp   Bead: uds-1xd6 (already claimed to you)

HAND-WRITTEN PACKET -- THE RENDERER IS DOWN AND YOU SHOULD KNOW WHAT THAT COSTS.
`omp-orchestrator dispatch render` exits 101: the binary does not compile right now.
  error[E0433]: cannot find type `DispatchAdmissibility` in this scope   main.rs:2075,2076,2091,2098
Another session is mid-refactor in that repo (packet_admission.rs already uses the type; main.rs
does not see it yet) with large uncommitted work. It is not ours and I did not touch it.
So this packet SKIPPED the renderer, which means it skipped the queue filter, the admissibility
check and the dispatch ledger. I substituted by hand: I confirmed pane %4 idle/safe_to_dispatch
via --robot-activity and claimed the bead to you with br update. The LEDGER ENTRY IS SIMPLY LOST
-- there is no record of this dispatch in the orchestrator's trail. Treat that as a known gap in
tonight's audit, not as something you need to repair.

## WHAT
`uds-f7vj` closed MUTATION-VERIFIED at d466419 with `acc5 UNRUN` recorded in the close reason. Its
acceptance 5 -- RECORD THE LANE IN THE RECEIPT -- was never done. This bead exists so that leg does
not live only inside a closed bead's reason string, where nothing ranks it and no one will find it.

## WHY IT WAS LEFT, AND WHY THAT WAS RIGHT
SageCastle: "f7vj acceptance 5 (record the lane in the receipt) is NOT DONE and left open on the bead
rather than quietly dropped." CrimsonCarp then graded and closed with `acc5 UNRUN` stated plainly
rather than rounded up. Both behaved correctly -- UNRUN with the leg named beats a false PASS. But a
closed bead is not a work queue: `br ready` cannot surface it, `bv` cannot rank it, and the only
record is a reason string. That is the gap this bead closes, and it is the same failure family as
uds-z69s -- state that exists but that nothing surfaces.

## THE SUBSTANCE
d466419 forwards `CARGO_LANE_KEYS` in the wrapper's precedence order plus ONE FIXED default lane
(`DEFAULT_CARGO_LANE`, a const -- never per-target, because a per-invocation lane is the
cp-cargo-lane-mint-leak class). So every uds build now runs under a KNOWN lane. Nothing writes that
lane into the receipt. A receipt that cannot say which lane produced the bytes cannot be audited
later: it records WHAT was built and not WHERE, and "where" is exactly the axis that has produced
wrong verdicts all night (uds-nqev: a stale in-source artifact shadowing fresh lane bytes, three of
four targets refusing `stale_artifact` about files the build never wrote).

## WHY THIS IS PROVENANCE, NOT BOOKKEEPING
uds-2fof made a false provenance claim inexpressible by re-keying on inputs. The lane is an input:
the same source at the same rev, built in two different lanes, can retrieve two different artifacts --
which is precisely what nqev demonstrated. A receipt that omits the lane is under-determined about
the bytes it certifies. This repo's standing rule is that a stamp that resolves nowhere is worse than
none, because a fabricated-looking provenance passes review on format alone.

## ACCEPTANCE
1. The receipt records the lane, and the RESOLUTION SOURCE for it -- operator-supplied via one of
   CARGO_LANE_KEYS, versus the `DEFAULT_CARGO_LANE` const. "uds" alone does not distinguish an
   operator who chose that lane from a default that filled it in.
2. NEGATIVE CONTROL: a receipt written when the lane cannot be resolved must say so by name -- typed
   and visible, never an empty string or a silent default. An absent lane rendered as a present one
   is the defect, not the fallback.
3. A planted receipt MISSING the lane field is REFUSED by name by whatever reads receipts. If nothing
   validates the field, adding it is decoration -- say so plainly rather than adding an unread field.
4. Existing receipts without the field must not be retro-labelled with a guessed lane. Absent is
   UNMEASURED; an invented attribution is worse than a gap, per uds-9v85.

## DO NOT
- Do not re-open uds-f7vj. It closed correctly on its other four legs, all mutation-proven with both
  failing BY NAME and a byte-identical restore (sha256 b80f693d98dabef9, 22 lib tests rc=0).
- Do not make the lane per-target or per-run while wiring it into the receipt. The fixed const is
  load-bearing and its rationale is in d466419.
- Do not touch ee's Cargo.lock (uds-q4gm owns it; a 342-line modification is already disclosed there).
- Capture every status UNPIPED: variable, then $? on the NEXT line.

HELD-SET (per your own uds-z69s, 00546a2 -- before work and after DONE):
  br list --assignee CrimsonCarp --status in_progress --json
Do NOT `br update --assignee ""` when you post DONE. DONE is a report; the grader closes.

CALLBACK:
  ntm --robot-send=control-plane --panes=1 --msg='DONE uds-1xd6 <PASS|BLOCKED|UNRUN> <sha> <one line>'
A different lineage grades this: you are Grok 4.6, so SageCastle (Opus 5), SapphireIsland or
CloudyGrove (GPT-5.6-Luna). You do not close your own.
```

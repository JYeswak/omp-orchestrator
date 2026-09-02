# Dispatch Journey Mapping Contract

Bead: `omp-orchestrator-dispatch-journey-mapping-42na`

## Purpose

This contract is the **mapping layer**, not a journey. `docs/plan/12-journey.md` is the canonical
A-to-Z spine and owns `S1`–`S9` plus the seven-field runbook contract; `docs/plan/11-lifecycle.md`
is the canonical evidence map and owns its §11.1 stage/owner table. Neither is superseded, amended,
or duplicated here, and applying the §12 runbook contract to this document is a scope error of the
kind §12's own scope note records.

What neither owns is the join: **no document here maps one operational dispatch stage to one
governing contract in `docs/contracts/` to one callable kernel symbol at a file and line.** §11.1
names owners and crate state in prose, citing zero contract files and zero symbols. This contract
supplies that join and names the measured gap — the load-bearing content, because one of nine stages
has no kernel at all and four more have kernels nothing calls. Every kernel claim was resolved
against `HEAD`, never the index; every GOAL resting on an external claim carries two independent
primary sources with fetch dates, and one resting only on in-repo measurement says so.

## Contract Artifacts

1. Canonical spine, **not owned here**: `docs/plan/12-journey.md` (`S1`–`S9`) and `docs/plan/11-lifecycle.md` (§11.1).
2. Canonical tick phases, **not owned here**: `orchestration_contract.md` — `OT-OBSERVE`, `OT-SELECT`, `OT-CLAIM`, `OT-DISPATCH`, `OT-RECEIPT`, `OT-VERIFY`, `OT-RECEIPT-EMIT`.
3. Runner: the command block in `## Validation`.
4. INVARIANT SUITE: **none exists.** No gate — a defect named in `## NO-CLAIM`, not disguised as pending.

## The mapping

Nine operational stages. `S` is the §12 parent; a §12 stage spans several operational stages (§11.1
records `S5` as "claim, dispatch, worker work, receiver receipt"), so this axis is finer than
`S1`–`S9` and does not renumber it. **Conventions:** contract paths are relative to
`docs/contracts/`, kernel paths relative to `crates/`, and `E`/`F` abbreviate `pub enum`/`pub fn`.
Every kernel anchor was resolved against `HEAD`.

| ID | stage | S | GOAL | governing contract | kernel symbol |
|---|---|---|---|---|---|
| `DJ-S-OBSERVE` | observe | S5 | Pane and work state from **two** independent surfaces, so no single confident field can authorize a send. | `pane_observation` (`PO-L1-TWO-CAPTURE-DOMINANCE`), `ground_truth`, `oracle_comparison` | `tick-monitor observe` at `tick-monitor/src/main.rs:24`; `tick-monitor/src/lifecycle.rs:35` `E Stage` |
| `DJ-S-SELECT` | select | S4→S5 | Choose from the whole candidate set *and* the panes that can receive it — never the newest row, never one candidate. | `admission` (`ADM-*` meet-semilattice), `pane_readiness` (`PR-*`), `degraded_dispatch_policy` | `pane-dispatch-ready/src/lib.rs:114` `E PaneDispatchReadyState{Free,Busy,QuotaBlocked,NoAgent,Unreadable}`; `loop-queue-filter/src/lib.rs:218` `F is_gated`, `:224` `F is_resource_blocked`; `admission-reason/src/lib.rs:24` `E Rule` |
| `DJ-S-CLAIM` | claim | S5 | Project ownership into the tracker **before** the packet leaves: the silence detector keys on `assigned ∧ in_progress`, so an unclaimed dispatch emits no signal at all. | `dispatch_claim` (`DCL-L1`…`L5`), `ack_spine` (`AS-L5-CLAIM-BEFORE-DISPATCH`) | `dispatch-claim-fence/src/lib.rs:279` `F authorize`, `:145` `E DispatchPermit` |
| `DJ-S-DISPATCH` | dispatch | S5 | Self-sufficient packet to a live pane, attempt durably recorded, so "which packet did this?" needs no human memory. | `orchestration` (`OT-DISPATCH`, `OC-L1`, `OC-L2`), `admission`, `degraded_dispatch_policy` | `ntm-fleet-monitor/src/bead_lifecycle.rs:180` `F dispatch(receipt: DispatchReceipt)`; `ack-spine/src/spine.rs` `DispatchIntent`, `PendingDispatch` |
| `DJ-S-ACK` | ack | S5→S6 | Transport, receiver delivery, and tracker acknowledgement stay three separately-evidenced authorities that never fill one another's column. | `ack_spine` (`AS-T-TRANSPORT`, `AS-D-DELIVERY`, `AS-A-ACK`, `AS-L1`…`L5`) | `ack-spine/src/authorities.rs:31` `E AckAuthority`; `ack-spine/src/followup.rs:86` `F classify_followup`; `ack-stage/src/lib.rs` `F assess`, `E TransportReceipt` |
| `DJ-S-RECEIPT` | receipt | S5 | Only the **receiver's own** post-send observation establishes arrival; a sender's success string is not delivery. | `receiver_receipt` (`RR-*`), `pane_observation` | `receiver-receipt/src/lib.rs:265` `F assess_receiver_receipt`, `E ReceiptVerdict`, `E ComposerEvidence` |
| `DJ-S-VERIFY` | verify | S6→S7 | Re-derive the cited command against current state; never read a report as evidence, never let a green run over an empty scan set count. | `verification` (`VC-WORKER-CLAIM`, `VC-BEAD-STATUS`, `VC-VERIFIED`, `VC-NO-EVIDENCE`) | `verify-dispatch/src/lib.rs:218` `F bead_status_via_br`; `ntm-fleet-monitor/src/bead_lifecycle.rs:208` `F verify_receiver`, `:242` `F grade` |
| `DJ-S-CLOSE` | close | S6→S8 | End ownership with a lifecycle event carrying re-runnable evidence; every named gap becomes a filed obligation, not prose. | `verification` (`VC-BEAD-STATUS`, sole completion authority), `finding` (`FC-*`) | `ntm-fleet-monitor/src/bead_lifecycle.rs:276` `F close(event_id: EventId)`; `finding/src/lib.rs:191` `F waive`, `:210`→`Filed`, `:168` `F spool` |
| `DJ-S-COMMS` | comms / mail | S5, S9 | Wake on a durable, restart-safe delivery cursor instead of re-reading buffers; carry the reservation/message leg of 1:many. | **NONE** — and because nothing governs it, its call convention is unwritten, which is the defect: that convention is load-bearing. | **NO IN-REPO KERNEL.** External primitives partly green, partly RED. See `DJ-G-COMMS-NO-KERNEL`. |

### `DJ-G-COMMS-NO-KERNEL` — the stage with no kernel, stated plainly

The comms/mail leg is the one stage with neither a governing contract nor an implementing crate —
not for lack of a kernel, but because the external kernel is only partly green and its call
convention is load-bearing and unwritten. Verdicts measured 2026-09-02 against a store that moved
from 133 projects/4,982 messages to 136/5,025 within the half hour: every figure needs its stamp.

**Provenance, corrected at source.** These `am` figures came from the **authenticated daemon**, not
a side-channel SQLite read: `cli/src/lib.rs:9017` defines
`should_use_daemon(direct, reachable) = !direct || reachable`, so omitting `--direct` takes the
daemon path unconditionally, and `:82140` reads a bearer token from `AGENT_MAIL_TOKEN` or
`HTTP_BEARER_TOKEN` — both set here for every command below. An earlier claim in this lane that the
CLI is token-less and reads `storage.sqlite3` directly is **retracted**; only `am health` builds a
throwaway probe database. `--direct` is inverted vs. its help text (`:9558`).

| primitive | verdict |
|---|---|
| `am inbox-events --agent <registered> --position-now` | **GREEN, fails closed.** Registered → cursor triple, rc=0; unregistered → `code:"inbox_events_unavailable"`, **rc=1**. A typed code *and* nonzero status — the shape for a monitor. |
| `oldest_available_cursor`, and a stored cursor below it | **NOT a floor, and my "silent clamp" is RETRACTED — see NO-CLAIM.** `sync.rs:574` makes it a per-recipient *first-event marker* (`MIN(seq)` per project+agent) inside one global `AUTOINCREMENT`; `sync.rs:606-624` refuses only once `global_oldest_cursor > 1`, so paging from the marker is the deliberate GH#238 fix. Never compare a stored cursor against it — that refuses healthy resumes from origin. Decode the daemon's typed `CURSOR_EXPIRED`. **A cursor is meaningless without its recipient.** |
| `ntm --robot-wait --wait-until=mail_pending` | **GREEN; RED only when externally killed.** Self-terminates at the documented 5 m default → `TIMEOUT` + `cursor_info{observed,next,oldest}`. Killed by an outside signal first → `CANCELED` with **`cursor_info` absent**. A supervisor that bounds the wait costs its caller the resume point. |
| `am robot search --project <repo> <query>` | **RED, whole corpus.** rc=1, `SQLite error: … column not found: m.topic`. A timeline fallback is a workaround, not a fix. |

A conforming caller MUST pass an explicit `--timeout` — not because the default never returns (it
does, at 300 s) but because five minutes is far too long for a dispatch loop and because **owning
the ceiling keeps the cursor**. It MUST treat `CANCELED` as a distinct typed variant, never as "no
mail": mapping it to an empty result violates `AS-L4-UNKNOWN-INHABITED`.

And the repository consumes none of it.
`grep -rlE 'agent-mail|agent_mail|inbox-events|mail_pending' crates/` returns exactly two files,
both non-calls: `loop-switch/src/lib.rs:7` (a doc comment) and `fleet-monitor/src/main.rs:442`
(`aux_lane(&cfg, "agent-mail-log-cap.sh", …)`, a **shell** lane in a repo whose `no-shell-gate`
forbids the extension). `wired-but-inert-guard/src/lib.rs:191` settles it: "comment-only mentions
never count as wiring." Absent from the plan too — the same grep over `docs/plan/11-lifecycle.md`
returns **0** across the whole evidence map, and the one `12-journey.md:792` hit is a skill name.

**So this is two gaps.** *Availability* — `inbox-events` and the blocking wake are green and simply
uncalled, the inverse of wired-but-inert; the larger half. *Capability* — search is dead
corpus-wide, and an externally-killed wait discards its cursor. **Two candidates were struck on
source review** (the below-floor page, the `mail_pending` default), so each survivor must resolve
one of three ways and say which: fixed at source and upstreamed, defended behind a typed refusal,
or a named finding with a reproduction. None stays a note.

## Measured defects in the existing mapping

| ID | defect | deriving command and result |
|---|---|---|
| `DJ-D-DISJOINT` | Three stage vocabularies coexist here and their three-way intersection of stable labels is **empty**. `loop_coverage::LoopLayer` (10) and `tick_monitor::lifecycle::Stage` (9) share **zero**; `LoopLayer` ∩ `LifecycleStatus` is empty on exact strings (`select`/`selected`, `verify`/`receiver_verified`). Only `{closed, grading}` is shared by any pair. | `comm -12` over the three `as_str()` sets: `loop-coverage/src/lib.rs:78`, `tick-monitor/src/lifecycle.rs:35`, `…/bead_lifecycle/model.rs:75` |
| `DJ-D-NO-ACK-MEMBER` | **No stage enum has an `ack`, `receipt`, or `mail` member.** `…/bead_lifecycle.rs` — the only in-tree machine walking select→dispatch→verify→grade→close — has **zero** occurrences of `ack`, `mail`, `inbox`, or `claim`; none of its 19 public methods is a claim or an ack. | `grep -cE 'ack\|mail\|inbox' …` → `0`; `claim\|Claim` → `0` |
| `DJ-D-OT-UNGATED` | The only contract carrying per-stage tick IDs cites, for all six laws and its Validation, a test target that **does not exist**: `no-shell-gate/tests/orchestration_tick.rs` is absent from disk *and* `HEAD`, as is its declared artifact `.flywheel/orchestration-ticks.jsonl`. Its own NO-CLAIM says "prose until the invariant suite exists"; it still is. | `cargo test -p no-shell-gate --test orchestration_tick` → **101**, `no test target named` |
| `DJ-D-KOP-FIVE` | `kernel_only_policy.md` names a kernel for exactly **five** capabilities (`KOP-C-OBSERVE`, `-DISPATCH`, `-TRIAGE`, `-BEAD`, `-SUBPROCESS`) — none for claim, ack, receipt, verify, close, or comms, so a handroll there is not even declarable. | `grep -oE 'KOP-C-[A-Z]+' … \| sort -u \| wc -l` → `5` |
| `DJ-D-CLAIM-HOMONYM` | "claim" is two unrelated nouns and nothing disambiguates them: `dispatch_claim_contract.md` means bead **ownership** (`DCL-V-CLAIMED`), `claim_strength_contract.md` means **evidence strength** (`CSL-*`). Routing by the word alone lands in the wrong contract. | `## Purpose` of each |
| `DJ-D-CENSUS` | A mention census over `docs/contracts/*.md` is not coverage; it misses in **both** directions — table below. | naive vs. stemmed / case-sensitive |
| `DJ-D-ZERO-CONSUMERS` | Four stage kernels have **zero** in-workspace consumers: `ack-spine` (only itself), `verify-dispatch`, `loop-queue-filter`, `admission-reason` — so `DJ-S-ACK` names a real, tested kernel nothing calls. `receiver-receipt` has three, `dispatch-claim-fence` one. | `grep -rlF <crate> --include=Cargo.toml crates/` |

Never quote one column. Denominator: the **20 pre-existing** contracts, excluding this file —
**the census is self-referential**: adding this document turns `comms 0` into `1`, `mail 3` into
`4`, `select 11` into `12`. A figure here is invalid unless it names the files counted.
`naive` is `grep -ril <stage>`; `concept` is stemmed, case-**sensitive** for type-shaped patterns:

| stage | naive | concept | why they differ |
|---|--:|--:|---|
| claim | 20 | 18 | every contract ends in a `## NO-CLAIM` heading |
| ack | 17 | **8** | nine hits were `package`, `tracker`, `read-back`, `background` |
| verify | 4 | **11** | naive *under*counts — "verification" lacks "verify" |
| observe | 13 | 19 | stem `observ` |
| close · select · dispatch · receipt | 16 · 11 · 18 · 14 | 14 · 11 · 18 · 14 | only `close` shifts (word-boundary) |
| mail · comms | 3 · 0 | 3 · 0 | the hole, under either method |

`-i` on a type-shaped pattern like `Ack[A-Z]` kills its case discrimination and matches `packet` —
which is how `ack` reached 17. One older finding is **retracted as stale**: round 22 recorded 19 of
50 `crates/*` as untracked; re-derived, `comm -13` of `git ls-tree -r HEAD -- crates` against
`ls -1 crates` is empty — **54 of 54 are in `HEAD`**.

## Research backing for the stage goals

Two independent primary sources per external claim, read as the arXiv abstract at each `abs/` URL.
Fetch date for all eight: **2026-09-01**.

- **`DJ-R-ALLOC`** → `DJ-S-SELECT`. **(1)** Amayuelas et al., *Self-Resource Allocation in
  Multi-Agent LLM Systems*, arXiv:2504.02051, 2025-04-02: the planner method beats the orchestrator
  method on concurrent actions, and *explicit worker-capability information* improves allocation
  "particularly when dealing with suboptimal workers" — i.e. `PaneDispatchReadyState` is a selection
  *input*, not a post-hoc check. **(2)** Liu et al., *Markets, Not Planners*, arXiv:2608.23867,
  2026-08-24: centralized single-planner allocation "creates a bottleneck as agent pools grow,
  requires private information … and can easily be manipulated, such that a single inserted
  preference nearly doubles a favored agent's task share under a centralized LLM allocator."
  Benchmark vs. mechanism design, one conclusion.

- **`DJ-R-RECEIPT`** → `DJ-S-RECEIPT`/`DJ-S-ACK`. **(1)** Andreakis, *Machine-Checked Dual-Write
  Recovery from a Committed Log*, arXiv:2608.00501, 2026-08-01 (Isabelle/HOL): an **information
  bound** — two reachable post-crash states share the same durable source-side state but differ in
  the sink's acceptance record, so "any recovery policy based only on the source side must duplicate
  an effect in one state or leave it undelivered in the other," while "an authoritative, complete,
  and current sink acceptance record lets recovery compute the missing operations."
  `AS-L1-NO-UPWARD-IMPLICATION` as a theorem. It also proves *arrival and claim fences*, arriving
  independently at the noun `dispatch-claim-fence` uses. **(2)** Figuera,
  *Notarized Agents: Receiver-Attested Confidential Receipts for AI Agent Actions*,
  arXiv:2606.04193, 2026-06-02: "the entity producing the activity log is the same entity whose
  activity is being logged," so the receiver signs a receipt of what *it* observed with its own key.
  The receipt is receiver-issued or it is not evidence.

- **`DJ-R-VERIFY`** → `DJ-S-VERIFY`/`DJ-S-CLOSE`. **(1)** Cemri et al., *Why Do Multi-Agent LLM
  Systems Fail?*, arXiv:2503.13657, 2025-03-17: MAST, from 150 expert-annotated traces (κ = 0.88)
  applied to 1600+ traces across 7 frameworks, yields 14 failure modes in **three** categories —
  system design, inter-agent misalignment, and **task verification**. Verification is a first-class
  failure category, not an afterthought. **(2)** Zhu et al., *Where LLM Agents Fail and How They
  can Learn From Failures*, arXiv:2509.25370, 2025-09-29: architectures "amplify vulnerability to
  cascading failures, where a single root-cause error propagates through subsequent decisions";
  isolating the root cause yields +24% all-correct and +17% step accuracy. Cascade is what makes a
  per-stage taxonomy load-bearing — without one, the observed failure is not the failing stage.

- **`DJ-R-COMMS`** → `DJ-S-COMMS`. **(1)** Sander et al., *A Technical Taxonomy of LLM Agent
  Communication Protocols*, arXiv:2606.19135, 2026-06-17: five dimensions — counterparty, payload,
  **interaction state**, **discovery mechanism**, schema flexibility — over nine maintained
  protocols; all sampled agent-to-agent protocols pair hybrid payloads with **session-state
  persistence**, and "decentralized discovery remains rare." A durable per-recipient cursor *is*
  the interaction-state dimension. **(2)** Louck et al., *Security Analysis of Agentic AI
  Communication Protocols*, arXiv:2511.03841, 2025-11-05: the first empirical comparative security
  analysis of these protocols, on a 14-point vulnerability taxonomy, concluding "existing protocols
  remain insufficiently secure." One taxonomic and one adversarial reading agree the layer needs its own contract.

**Not externally sourced, and labelled as such.** `DJ-S-OBSERVE`, `DJ-S-CLAIM`, and `DJ-S-DISPATCH`
rest **only** on in-repo measurement (`pane_observation_contract.md`'s two-capture floor; `OC-L2`'s
131 re-dispatches over 247 minutes; `AS-L5`'s `5rh` failure). No external source was verified for
them this pass, so they are single-lineage — **phantom doctrine with respect to the literature** —
and must not be cited as corroborated until a second primary source is recorded here.

## Validation

```bash
cd /Users/josh/Developer/omp-orchestrator
D=docs/contracts/dispatch_journey_mapping_contract.md
# shape: <25000 bytes, 6 required headings each exactly once, >= 8 DJ-* ids
wc -c "$D"; grep -oE 'DJ-[A-Z][A-Z-]+' "$D" | sort -u | wc -l
for h in Purpose "Contract Artifacts" Validation Cross-References Non-Coverage NO-CLAIM; do
  printf '%-20s %s\n' "$h" "$(grep -cxF "## $h" "$D")"   # each MUST be 1
done
# every kernel anchor resolves in HEAD (not the index) and the named symbol is present
for a in tick-monitor/src/lifecycle.rs:'pub enum Stage' \
  pane-dispatch-ready/src/lib.rs:'pub enum PaneDispatchReadyState' \
  dispatch-claim-fence/src/lib.rs:'pub fn authorize' \
  ntm-fleet-monitor/src/bead_lifecycle.rs:'pub fn close' \
  ack-spine/src/authorities.rs:'pub enum AckAuthority' \
  receiver-receipt/src/lib.rs:'pub fn assess_receiver_receipt' \
  verify-dispatch/src/lib.rs:'pub fn bead_status_via_br' \
  loop-coverage/src/lib.rs:'pub enum LoopLayer' ; do
  f=crates/${a%%:*}; s=${a#*:}
  test -n "$(git ls-tree -r HEAD --name-only -- "$f")" && grep -qF "$s" "$f" \
    && echo "OK   $f $s" || echo "FAIL $f $s"
done
# DJ-D-OT-UNGATED known-bad leg: assert the MESSAGE, never the bare exit code
RCH_ENABLED=false CARGO_MINT_MIN_CONTAINER_PCT=0 cargo test --quiet -p no-shell-gate \
  --test orchestration_tick 2>&1 | grep -qF "no test target named \`orchestration_tick\`" \
  && echo "OT-UNGATED still true" || echo "OT-UNGATED changed - re-verify the cause"
```

Anti-vacuity: the anchor and heading loops iterate hard-coded lists, so an empty scan set is
impossible and each iteration prints a named `OK`/`FAIL`/count. The `orchestration_tick` leg is
**fires-on-known-bad** and inverts on repair. It greps the **message**, not the exit code: during a
workspace outage the same command also exited 101 from an unrelated cause, and `101` is `cargo`'s
generic failure while the message is the finding.

## Cross-References

Contract paths relative to `docs/contracts/`, kernel paths relative to `crates/`.

- `docs/plan/12-journey.md` — **canonical** `S1`–`S9` spine; this doc is subordinate · `docs/plan/11-lifecycle.md` — **canonical** evidence map, §11.1
- `orchestration_contract.md` (`OT-*`, `OC-L1`–`L6`, `OC-A1`–`A4`) · `dispatch_claim_contract.md` (`DCL-L1`–`L5`) · `ack_spine_contract.md` (`AS-L1`–`L5`) · `receiver_receipt_contract.md` · `verification_contract.md` · `pane_observation_contract.md` (`PO-L1`) · `finding_contract.md`
- `pane_readiness_contract.md` · `admission_contract.md` · `degraded_dispatch_policy.md` — selection inputs · `kernel_only_policy.md` (`KOP-C-*`) · `claim_strength_contract.md` — the *other* "claim"
- `loop-coverage/src/lib.rs:78` · `tick-monitor/src/lifecycle.rs:35` · `ntm-fleet-monitor/src/bead_lifecycle/model.rs:75` — the three disjoint vocabularies · `…/bead_lifecycle.rs` — the only end-to-end machine · `wired-but-inert-guard/src/lib.rs:191`
- `mcp_agent_mail_rust@c7a7083f` `mcp-agent-mail-db/src/sync.rs:574,606-624` and `mcp-agent-mail-cli/src/lib.rs:9017,9558,82140` — the definitions that refuted my clamp and CLI claims

## Non-Coverage

- **`DJ-S-COMMS` has zero contract coverage and zero in-repo kernel.** Not partial, not thin: nothing in `docs/contracts/` governs inter-agent messaging and no crate calls `am` or `ntm --wait-until=mail_pending`. The three contracts containing `mail` mention Agent Mail only to disclaim it.
- **`DJ-S-ACK` has a strong contract and a tested kernel with zero consumers.** `ack-spine` is depended on by nothing but itself, `AS-L4`/`AS-L5` are recorded gaps in their own contract, and no stage enum has an `ack` member. A strong contract is not coverage of the stage.
- **`DJ-S-VERIFY` and `DJ-S-CLOSE` are the weakest *modelled* stages.** `verify-dispatch` has zero in-workspace consumers; `bead_lifecycle::grade`/`close` exist but reach production through no verified path.
- **`DJ-S-SELECT` is split across three contracts and no crate joins them.** `loop-queue-filter` and `admission-reason` have zero consumers; `pane-dispatch-ready` is consumed only by `reap-finished-panes` — a reaper, not a selector.
- No tenth stage, no new stage enum, no bridge between the three existing ones, no `S10`. Naming `DJ-D-DISJOINT` is not fixing it. Nothing in `docs/` or `crates/` is edited here.
- No claim that any mapped kernel is *called in production* — symbol existence in `HEAD` is all that was verified; `DJ-D-ZERO-CONSUMERS` is the honest half. The 1:many question is out of scope.

## NO-CLAIM

**This document has no gate, and that is a defect, not a phase.** There is no
`crates/*/tests/dispatch_journey_mapping.rs`, so nothing stops this mapping from drifting from the
crates it cites. `## Validation` is commands someone must choose to run, and `DJ-D-OT-UNGATED` is
exactly the failure of trusting a contract's cited test to exist: this document is in the same class
as the contract it criticizes, differing only by admitting it.

A resolved anchor proves a symbol is present at a line in `HEAD` — not that it is reachable, called,
or correct, nor that the stage executes. Nine rows are nine verified anchors and one honest absence,
not a working lifecycle. `DJ-D-CENSUS` measures *file mentions*; even the concept column is a proxy.
The research section shows four goals have independent external support, not that the eight papers
endorse this design, that their results transfer to a tmux pane fleet, or that any was read past its
abstract. `DJ-S-OBSERVE`, `DJ-S-CLAIM`, `DJ-S-DISPATCH` stay phantom.

The comms verdicts began as probe results against a Homebrew `am 0.3.31` and the installed `ntm`,
neither built from this workspace. **Four readings in this section were wrong and every one was
refuted by a second reader:** `oldest_available_cursor` as an eviction floor (it is a per-recipient
first-event marker); the externally-killed wait as unbounded (the 5-minute default is real); the
below-floor page as a silent clamp (it is the deliberate GH#238 fix, and the docstring I called a
broken promise is true); and the CLI as a token-less SQLite reader (it takes the authenticated
daemon by default). In each case the *measurement* was sound and the *diagnosis* skipped reading the
code that defines correct behaviour — three times a source comment or predicate said exactly why.
**Before calling anything a defect, read the definition of correct behaviour.** Three corollaries,
each paid for: a bounded observation **cannot establish unboundedness** (every ceiling was under the
300 s deadline); a **guard is a claim** and inherits the rule, so a guard against a misdiagnosed
defect is worse than none because it refuses healthy traffic while advertising protection; and a
**borrowed claim inherits its author's burden** — the CLI-provenance error entered this document
because I repeated a peer's measured-sounding finding without reading the source myself, which is
`OC-A4` (a report is a claim) applied to a sibling rather than to a subagent.
